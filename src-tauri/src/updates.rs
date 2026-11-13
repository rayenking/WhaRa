//! Verified, in-app macOS updates. Only the local update window can install.
//! The remote WhatsApp origin never receives updater permissions.

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_updater::UpdaterExt;

use crate::release;

pub const WINDOW: &str = "updates";
const FIRST_CHECK: Duration = Duration::from_secs(40);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const REMIND_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub notes: Option<String>,
    pub available: bool,
    pub checked: bool,
    pub installing: bool,
    pub error: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Reminder {
    dismissed_version: Option<String>,
    remind_at_ms: Option<u64>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|span| span.as_millis() as u64)
        .unwrap_or(0)
}

fn is_snoozed(reminder: &Reminder, version: &str, at_ms: u64) -> bool {
    reminder.dismissed_version.as_deref() == Some(version)
        && reminder.remind_at_ms.unwrap_or(0) > at_ms
}

pub struct UpdaterState {
    info: Mutex<UpdateInfo>,
    dismissed: Mutex<Reminder>,
}

fn reminder_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("updater-reminder.json"))
}

fn read_reminder(app: &AppHandle) -> Reminder {
    reminder_path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_reminder(app: &AppHandle, reminder: &Reminder) -> Result<(), String> {
    let path = reminder_path(app).ok_or("Cannot locate WhaRa application support")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }
    let raw = serde_json::to_string_pretty(reminder).map_err(|err| err.to_string())?;
    std::fs::write(path, raw).map_err(|err| err.to_string())
}

pub fn bootstrap(app: &AppHandle) {
    app.manage(UpdaterState {
        info: Mutex::new(UpdateInfo {
            current: release::VERSION.to_string(),
            ..UpdateInfo::default()
        }),
        dismissed: Mutex::new(read_reminder(app)),
    });
}

fn snapshot(app: &AppHandle) -> UpdateInfo {
    app.state::<UpdaterState>()
        .info
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| UpdateInfo {
            current: release::VERSION.to_string(),
            ..UpdateInfo::default()
        })
}

pub fn updater_installing(app: &AppHandle) -> bool {
    snapshot(app).installing
}

fn set_info(app: &AppHandle, info: UpdateInfo) {
    if let Ok(mut stored) = app.state::<UpdaterState>().info.lock() {
        *stored = info.clone();
    }
    if let Some(panel) = app.get_webview_window(WINDOW) {
        panel.emit("updater-state", info.clone()).ok();
    }
    if let Some(panel) = app.get_webview_window(crate::shell::PREFS) {
        panel.emit("updater-state", info).ok();
    }
}

fn show_prompt_now(app: &AppHandle) -> Result<(), String> {
    if let Some(panel) = app.get_webview_window(WINDOW) {
        panel.show().map_err(|err| err.to_string())?;
        panel.set_focus().ok();
        return Ok(());
    }
    WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("update.html".into()))
        .title("WhaRa Update")
        .inner_size(440.0, 280.0)
        .resizable(false)
        .center()
        .build()
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn show_prompt(app: &AppHandle) -> Result<(), String> {
    // AppKit / WKWebView window creation must happen on the macOS main thread,
    // not directly in the async updater's network-check thread.
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(reason) = show_prompt_now(&handle) {
            eprintln!("[whara] cannot show update prompt: {reason}");
        }
    })
    .map_err(|err| err.to_string())
}

/// Check signed-release metadata, never the WhatsApp Web origin.
async fn scan(app: &AppHandle, manual: bool) -> Result<UpdateInfo, String> {
    // Avoid accidentally replacing a debug binary with a packaged application.
    if cfg!(debug_assertions) {
        return Err("In-app installation is disabled in development builds.".into());
    }

    let result = app.updater().map_err(|err| err.to_string())?.check().await;

    match result {
        Ok(found) => {
            let mut info = snapshot(app);
            info.error = None;
            info.checked = true;
            info.latest = found.as_ref().map(|update| update.version.clone());
            info.notes = found.as_ref().and_then(|update| update.body.clone());
            info.available = found.is_some();
            set_info(app, info.clone());

            if let Some(update) = found {
                let dismissed = app
                    .state::<UpdaterState>()
                    .dismissed
                    .lock()
                    .map(|state| is_snoozed(&state, &update.version, now_ms()))
                    .unwrap_or(false);
                if manual || !dismissed {
                    show_prompt(app)?;
                }
            }
            Ok(info)
        }
        Err(err) => {
            let mut info = snapshot(app);
            info.error = Some(err.to_string());
            set_info(app, info.clone());
            Err(err.to_string())
        }
    }
}

/// Poll after launch and every 6h; network failures never block WhatsApp.
pub fn schedule(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if let Err(err) = tauri::async_runtime::block_on(scan(&app, false)) {
                eprintln!("[whara] in-app update check: {err}");
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
}

#[tauri::command]
pub fn updater_state(app: AppHandle) -> UpdateInfo {
    snapshot(&app)
}

/// Manual check is available in Preferences, even for a version previously deferred.
#[tauri::command]
pub async fn updater_check_now(app: AppHandle) -> Result<UpdateInfo, String> {
    scan(&app, true).await
}

/// Do not remind again about this specific version; future versions can prompt.
#[tauri::command]
pub fn updater_later(app: AppHandle) -> Result<(), String> {
    remember_later(&app)?;
    if let Some(window) = app.get_webview_window(WINDOW) {
        window.close().map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Treat the native red close button exactly like "Later".
pub fn remember_later(app: &AppHandle) -> Result<(), String> {
    if let Some(version) = snapshot(app).latest {
        let state = app.state::<UpdaterState>();
        let mut reminder = state.dismissed.lock().map_err(|err| err.to_string())?;
        reminder.dismissed_version = Some(version);
        reminder.remind_at_ms = Some(now_ms().saturating_add(REMIND_AFTER.as_millis() as u64));
        save_reminder(app, &reminder)?;
    }
    Ok(())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    downloaded: u64,
    total: Option<u64>,
    phase: &'static str,
}

fn progress(app: &AppHandle, downloaded: u64, total: Option<u64>, phase: &'static str) {
    app.emit_to(
        WINDOW,
        "updater-progress",
        InstallProgress {
            downloaded,
            total,
            phase,
        },
    )
    .ok();
}

/// Download signed updater archive, install, and relaunch (macOS requirement).
#[tauri::command]
pub async fn updater_install(app: AppHandle) -> Result<(), String> {
    let state = app.state::<UpdaterState>();
    {
        let mut info = state.info.lock().map_err(|err| err.to_string())?;
        if info.installing {
            return Err("An update is already installing.".into());
        }
        if !info.available {
            return Err("No verified update has been found.".into());
        }
        info.installing = true;
    }
    set_info(&app, snapshot(&app));

    let outcome = install(&app).await;
    if let Err(err) = &outcome {
        let mut info = snapshot(&app);
        info.installing = false;
        info.error = Some(err.clone());
        set_info(&app, info);
    }
    outcome
}

async fn install(app: &AppHandle) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Installing updates from a debug build is disabled.".into());
    }
    let expected = snapshot(app).latest.ok_or("Missing update metadata")?;
    progress(app, 0, None, "checking");
    let update = app
        .updater()
        .map_err(|err| err.to_string())?
        .check()
        .await
        .map_err(|err| err.to_string())?
        .ok_or("The update is no longer available")?;
    if update.version != expected {
        return Err("A different release is now available. Check for updates again.".into());
    }

    let downloaded = std::sync::atomic::AtomicU64::new(0);
    update
        .download_and_install(
            |chunk_size, content_length| {
                let received = downloaded
                    .fetch_add(chunk_size as u64, std::sync::atomic::Ordering::Relaxed)
                    + chunk_size as u64;
                progress(app, received, content_length, "downloading");
            },
            || {
                progress(
                    app,
                    downloaded.load(std::sync::atomic::Ordering::Relaxed),
                    None,
                    "installing",
                )
            },
        )
        .await
        .map_err(|err| err.to_string())?;
    progress(
        app,
        downloaded.load(std::sync::atomic::Ordering::Relaxed),
        None,
        "restarting",
    );
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_update_state_does_not_offer_unverified_update() {
        let info = UpdateInfo::default();
        assert!(!info.available);
        assert!(!info.installing);
    }

    #[test]
    fn later_snoozes_for_one_day_but_not_new_versions() {
        let reminder = Reminder {
            dismissed_version: Some("0.2.1".into()),
            remind_at_ms: Some(1000 + REMIND_AFTER.as_millis() as u64),
        };
        assert!(is_snoozed(&reminder, "0.2.1", 2000));
        assert!(!is_snoozed(
            &reminder,
            "0.2.1",
            reminder.remind_at_ms.unwrap()
        ));
        assert!(!is_snoozed(&reminder, "0.2.2", 2000));
    }
}
