//! Commands callable from the injected page script and the preferences window.

use serde_json::Value;
use tauri::AppHandle;
use tauri::Emitter as Broadcaster;
use tauri::Manager as Registry;

use crate::config::{self, Preferences};
use crate::hotkeys;
use crate::refresh::{self, UpdateStatus};
use crate::release;
use crate::{shell, startup, tray};

/// Read the stored preferences.
#[tauri::command]
pub fn read_config(state: tauri::State<'_, config::Store>) -> Preferences {
    match state.0.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => Preferences::default(),
    }
}

/// Merge a partial preferences object over the stored values and persist it.
///
/// Applying the change is part of saving it: the hotkey is rebound and the
/// login item resynced, so the settings page never reports something the
/// system has not actually taken on.
#[tauri::command]
pub fn store_config(app: AppHandle, patch: Value) -> Result<Preferences, String> {
    let before = config::snapshot(&app);
    let mut merged = before.clone();
    let base = serde_json::to_value(&merged).map_err(|err| err.to_string())?;

    if let (Value::Object(base), Value::Object(patch)) = (base, patch) {
        let combined = Value::Object(base.into_iter().chain(patch).collect());
        merged = serde_json::from_value(combined).map_err(|err| err.to_string())?;
    }

    let saved = config::store(&app, merged)?;

    // Rebind only when the key actually changed, so unrelated saves do not
    // drop and re-add the shortcut.
    if saved.blur_hotkey != before.blur_hotkey {
        hotkeys::assign(&app, &saved.blur_hotkey).map_err(|err| err.to_string())?;
    }

    // Same for the login item.
    if saved.launch_at_login != before.launch_at_login {
        startup::sync(&app, saved.launch_at_login).map_err(|err| err.to_string())?;
    }

    push_live_state(&app, &saved);
    Ok(saved)
}

/// Invoked by the page script whenever the unread badge changes.
#[tauri::command]
pub fn report_unread(app: AppHandle, total: u32) {
    tray::display_unread(&app, total);
}

#[tauri::command]
pub fn flip_blur(app: AppHandle) {
    shell::flip_privacy_blur(&app);
}

#[tauri::command]
pub fn apply_zoom(app: AppHandle, zoom: f64) {
    shell::set_interface_zoom(&app, zoom.clamp(0.6, 2.0));
}

#[tauri::command]
pub fn reveal_preferences(app: AppHandle) {
    shell::open_preferences(&app);
}

#[tauri::command]
pub fn raise_chat(app: AppHandle) {
    shell::resume_active_session(&app);
}

/// Version of the running build, for the preferences footer.
#[tauri::command]
pub fn build_info() -> release::Banner {
    release::banner()
}

/// Current update status, without touching the network.
#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateStatus {
    refresh::status(&app)
}

/// Check the release feed now and return the fresh status.
#[tauri::command]
pub fn check_update(app: AppHandle) -> Result<UpdateStatus, String> {
    refresh::check(&app, true)
}

/// Open a release page in the default browser.
#[tauri::command]
pub fn open_release(url: String) -> Result<(), String> {
    refresh::open_page(&url)
}

/// Push preferences into the live webview and the preferences window.
pub fn push_live_state(app: &AppHandle, prefs: &Preferences) {
    shell::set_interface_zoom(app, prefs.zoom);
    shell::set_privacy_blur(app, prefs.privacy_blur);

    if let Some(panel) = app.get_webview_window(shell::PREFS) {
        panel.emit("preferences-updated", prefs.clone()).ok();
    }
}

/// Apply whatever is on disk right after startup.
pub fn push_stored_state(app: &AppHandle) {
    let prefs = config::snapshot(app);
    push_live_state(app, &prefs);
}
