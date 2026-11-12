//! Chat window creation plus the small preferences window.

use tauri::webview::NewWindowResponse;
use tauri::AppHandle;
use tauri::Manager as Registry;
use tauri::WebviewUrl;
use tauri::WebviewWindow;
use tauri::WebviewWindowBuilder;

pub const CHAT: &str = "chat";
pub const PREFS: &str = "prefs";

const WHATSAPP_HOME: &str = "https://web.whatsapp.com/";
/// WhatsApp Web gates login on a recent engine and rejects the bare WKWebView
/// user agent. The engine really is Safari, so the panel advertises the Safari
/// release that ships with the running macOS.
#[cfg(target_os = "macos")]
fn browser_identity() -> String {
    safari_identity(mac_os_major())
}

/// macOS major version, 0 when it cannot be read.
#[cfg(target_os = "macos")]
fn mac_os_major() -> u64 {
    use objc2_foundation::NSProcessInfo;

    NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion as u64
}

/// Safari freezes the OS token at `10_15_7` while its own version tracks the OS:
/// the major plus three up to macOS 15, then equal from macOS 26 on, when both
/// switched to year-based numbering.
#[cfg(target_os = "macos")]
fn safari_identity(mac_os_major: u64) -> String {
    let safari = if mac_os_major >= 26 {
        mac_os_major
    } else {
        mac_os_major.max(11) + 3
    };

    format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/{safari}.0 Safari/605.1.15"
    )
}

/// Load both independent page layers. The native WhatsApp internals remain
/// in their original DOM; the second layer adds reversible UI controls.
#[cfg(all(target_os = "macos", debug_assertions))]
fn injected_js() -> String {
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/whara-inject.js"
    ))
    .expect("failed to read injected WhatsApp bridge");
    let ui = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/whara-ui.js"
    ))
    .expect("failed to read injected WhaRa UI");
    format!("{base}\n{ui}")
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn injected_js() -> String {
    format!(
        "{}\n{}",
        include_str!("../resources/whara-inject.js"),
        include_str!("../resources/whara-ui.js"),
    )
}

/// Create (or return) the main WhatsApp panel.
pub fn mount_chat(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(panel) = app.get_webview_window(CHAT) {
        return Ok(panel);
    }

    let home: tauri::Url = WHATSAPP_HOME.parse().expect("static WhatsApp URL");

    let mut builder = WebviewWindowBuilder::new(app, CHAT, WebviewUrl::External(home))
        .title("WhaRa")
        .inner_size(1120.0, 800.0)
        // Match the observed Telegram compact window dimensions (470 x 740)
        // on this Mac. Single-pane WhatsApp navigation activates below 800 px.
        .min_inner_size(470.0, 740.0)
        .center()
        .initialization_script(injected_js());

    // WhatsApp Web gates login on a recent engine and rejects the bare WKWebView
    // user agent. The engine really is Safari, so the panel advertises the Safari
    // release that ships with the running macOS.
    builder = builder.user_agent(&browser_identity());

    // WhatsApp asks for a pop-up in two situations that matter here: the
    // "Open device settings" button in the notification help panel, and any
    // auth or payment sheet. Tauri denies those by default, which is why the
    // button looked dead. Send them to the real browser instead of dropping
    // them on the floor.
    builder = builder.on_new_window(|url, _features| match url.scheme() {
        "http" | "https" => {
            crate::refresh::open_page(url.as_str()).ok();
            NewWindowResponse::Deny
        }
        _ => NewWindowResponse::Deny,
    });

    let panel = builder.build()?;

    // Keep Web Inspector available while iterating on the remote WhatsApp DOM.
    // Do not expose it in release builds.
    #[cfg(debug_assertions)]
    panel.open_devtools();

    Ok(panel)
}

/// Bring the chat to the front, creating it if it was closed for good.
pub fn resume_active_session(app: &AppHandle) {
    match app.get_webview_window(CHAT) {
        Some(panel) => {
            panel.show().ok();
            panel.unminimize().ok();
            panel.set_focus().ok();
        }
        None => {
            if let Ok(panel) = mount_chat(app) {
                panel.set_focus().ok();
            }
        }
    }
}

pub fn minimise_to_menubar(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(CHAT) {
        panel.hide().ok();
    }
}

pub fn toggle_chat(app: &AppHandle) {
    match app.get_webview_window(CHAT) {
        Some(panel) if panel.is_visible().unwrap_or(false) => {
            panel.hide().ok();
        }
        Some(panel) => {
            panel.show().ok();
            panel.set_focus().ok();
        }
        None => resume_active_session(app),
    }
}

/// Open the local preferences page, reusing the window when it exists.
pub fn open_preferences(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(PREFS) {
        panel.show().ok();
        panel.set_focus().ok();
        return;
    }

    WebviewWindowBuilder::new(app, PREFS, WebviewUrl::App("index.html".into()))
        .title("WhaRa Preferences")
        .inner_size(520.0, 620.0)
        .center()
        .build()
        .ok();
}

/// Execute JavaScript inside the WhatsApp view; ignore failures while loading.
pub fn run_script(app: &AppHandle, source: &str) {
    if let Some(panel) = app.get_webview_window(CHAT) {
        panel.eval(source).ok();
    }
}

pub fn set_interface_zoom(app: &AppHandle, zoom: f64) {
    if let Some(panel) = app.get_webview_window(CHAT) {
        // Use WebKit's page zoom rather than CSS `zoom` on <html>. CSS zoom
        // changes the coordinate/layout space seen by WhatsApp's virtualized
        // lists and responsive panes; native zoom leaves the DOM layout engine
        // and its viewport calculations in control.
        panel.set_zoom(zoom.clamp(0.6, 2.0)).ok();
    }
}

pub fn set_privacy_blur(app: &AppHandle, active: bool) {
    run_script(
        app,
        &format!("window.__whara && window.__whara.setBlur({active});"),
    );
}

pub fn flip_privacy_blur(app: &AppHandle) {
    run_script(app, "window.__whara && window.__whara.toggleBlur();");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Safari's version tracks the OS, so the token must move with it.
    #[cfg(target_os = "macos")]
    #[test]
    fn safari_token_follows_the_operating_system() {
        assert!(safari_identity(15).contains("Version/18.0 Safari/605.1.15"));
        assert!(safari_identity(26).contains("Version/26.0 Safari/605.1.15"));
    }

    /// The frozen OS token is what makes WhatsApp accept the request.
    #[cfg(target_os = "macos")]
    #[test]
    fn os_token_stays_frozen() {
        assert!(safari_identity(26)
            .starts_with("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15"));
    }
}
