//! Local-only macOS debugging interface.
//!
//! Enabled exclusively in debug builds. A UNIX socket under the current user's
//! private ~/Library/Caches/whara-dev-bridge directory lets development scripts
//! evaluate JavaScript in the *existing* WKWebView, without opening Web Inspector
//! or creating an IPC command accessible to the WhatsApp Web page.
//!
//! WARNING: eval can access the logged-in WhatsApp DOM, including private data.
//! Keep results local, never run scripts from untrusted sources, and never ship
//! this bridge in a release build.

use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::mpsc,
    thread,
    time::Duration,
};

use block2::RcBlock;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSError, NSString};
use objc2_web_kit::WKWebView;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, LogicalSize, Manager};

const SOCKET_NAME: &str = "bridge.sock";
const MAX_REQUEST_BYTES: u64 = 512 * 1024;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const EVAL_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Deserialize)]
struct Request {
    op: String,
    code: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
}

fn socket_directory() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Caches")
        .join("whara-dev-bridge"))
}

pub fn start(app: &AppHandle) -> io::Result<()> {
    let directory = socket_directory()?;
    fs::create_dir_all(&directory)?;
    // Only processes running as the current macOS user can connect.
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let path = directory.join(SOCKET_NAME);

    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            eprintln!("[whara-dev] another debug bridge is already listening");
            return Ok(());
        }
        fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;

    let app = app.clone();
    thread::Builder::new()
        .name("whara-dev-bridge".into())
        .spawn(move || {
            eprintln!("[whara-dev] local inspector ready (no Web Inspector window)");
            for incoming in listener.incoming() {
                match incoming {
                    Ok(stream) => {
                        if let Err(err) = serve(&app, stream) {
                            eprintln!("[whara-dev] request error: {err}");
                        }
                    }
                    Err(err) => eprintln!("[whara-dev] socket error: {err}"),
                }
            }
        })?;
    Ok(())
}

fn serve(app: &AppHandle, mut stream: UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let mut buffer = String::new();
    let count = BufReader::new(stream.try_clone()?)
        .take(MAX_REQUEST_BYTES + 1)
        .read_line(&mut buffer)?;
    if count == 0 {
        return Ok(());
    }

    let answer = if count as u64 > MAX_REQUEST_BYTES {
        json!({"ok":false,"error":"request exceeds size limit"})
    } else {
        match serde_json::from_str::<Request>(&buffer) {
            Ok(request) => process(app, request),
            Err(error) => json!({"ok":false,"error":format!("invalid request: {error}")}),
        }
    };

    let mut encoded = serde_json::to_vec(&answer)?;
    if encoded.len() > MAX_RESPONSE_BYTES {
        encoded = serde_json::to_vec(
            &json!({"ok":false,"error":"result too large; narrow the DOM query"}),
        )?;
    }
    encoded.push(b'\n');
    stream.write_all(&encoded)?;
    Ok(())
}

fn process(app: &AppHandle, request: Request) -> Value {
    match request.op.as_str() {
        "ping" => json!({
            "ok":true,
            "debugOnly":true,
            "transport":"private Unix socket",
            "window":crate::shell::CHAT
        }),
        "eval" | "exec" => {
            let Some(code) = request.code else {
                return json!({"ok":false,"error":"missing JavaScript code"});
            };
            if code.len() > MAX_REQUEST_BYTES as usize {
                return json!({"ok":false,"error":"JavaScript exceeds size limit"});
            }
            match evaluate(app, &code, request.op == "exec") {
                Ok(value) => value,
                Err(error) => json!({"ok":false,"error":error}),
            }
        }
        "size" => {
            let (Some(width), Some(height)) = (request.width, request.height) else {
                return json!({"ok":false,"error":"width and height are required"});
            };
            if !width.is_finite()
                || !height.is_finite()
                || !(470.0..=2560.0).contains(&width)
                || !(740.0..=1800.0).contains(&height)
            {
                return json!({"ok":false,"error":"size out of allowed window range"});
            }
            match app.get_webview_window(crate::shell::CHAT) {
                Some(panel) => match panel.set_size(LogicalSize::new(width, height)) {
                    Ok(()) => json!({"ok":true,"width":width,"height":height}),
                    Err(err) => json!({"ok":false,"error":err.to_string()}),
                },
                None => json!({"ok":false,"error":"chat window not found"}),
            }
        }
        _ => json!({"ok":false,"error":"unknown operation"}),
    }
}

/// JS can only execute in the official WhatsApp origin.
fn trusted_whatsapp_host(host: &str) -> bool {
    host == "web.whatsapp.com"
}

fn wrap_script(code: &str, statements: bool) -> String {
    // In WhatsApp Web, Content Security Policy disallows JS eval() and
    // new Function(). Embedding the source directly into native WebKit's
    // evaluateJavaScript avoids both and works with the page's strict CSP.
    let source = if statements {
        format!("(function() {{\n{code}\n}})()")
    } else {
        format!("({code})")
    };
    // The native completion callback always receives a JSON string.
    format!(
        "(function() {{ try {{ const value = {source}; \
         return JSON.stringify({{ok:true,result:value === undefined ? null : value}}); \
         }} catch (err) {{ return JSON.stringify({{ok:false,error:String(err)}}); }} }})()"
    )
}

fn evaluate(app: &AppHandle, code: &str, statements: bool) -> Result<Value, String> {
    let panel = app
        .get_webview_window(crate::shell::CHAT)
        .ok_or("chat window is not running")?;
    let url = panel.url().map_err(|err| err.to_string())?;
    if url.scheme() != "https" || !url.host_str().is_some_and(trusted_whatsapp_host) {
        return Err("refusing to evaluate outside https://web.whatsapp.com".into());
    }

    let javascript = wrap_script(code, statements);
    let (sender, receiver) = mpsc::sync_channel::<Result<String, String>>(1);
    panel
        .with_webview(move |native| {
            let handler = RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
                let result = unsafe {
                    if !error.is_null() {
                        Err((*error).localizedDescription().to_string())
                    } else if value.is_null() {
                        Err("WebKit returned no result".into())
                    } else {
                        // wrap_script always returns JSON encoded as a JS String.
                        let string = &*(value as *const NSString);
                        Ok(string.to_string())
                    }
                };
                let _ = sender.send(result);
            });

            unsafe {
                let webview: &WKWebView = &*native.inner().cast();
                webview.evaluateJavaScript_completionHandler(
                    &NSString::from_str(&javascript),
                    Some(&handler),
                );
            }
        })
        .map_err(|err| err.to_string())?;

    let response = receiver
        .recv_timeout(EVAL_TIMEOUT)
        .map_err(|_| "WebKit timed out; check whether WhatsApp Web is still loading")??;
    serde_json::from_str(&response).map_err(|err| format!("invalid WebKit response: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_only_trusts_whatsapp_web() {
        assert!(trusted_whatsapp_host("web.whatsapp.com"));
        assert!(!trusted_whatsapp_host("evil.whatsapp.com"));
        assert!(!trusted_whatsapp_host("whatsapp.com.attacker.test"));
    }

    #[test]
    fn script_encoding_preserves_quotes_and_newlines() {
        let js = wrap_script("const x = \"quoted\";\nx", true);
        assert!(js.contains("const x = \"quoted\";"));
        assert!(js.contains("\nx"));
        assert!(!js.contains("eval("));
    }
}
