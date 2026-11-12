//! Launch-at-login, backed by the autostart plugin.
//!
//! The toggle is stored with the other preferences, and this module is what
//! turns that stored flag into a real macOS login item. Without it the setting
//! would only ever be a value in a JSON file.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as Launcher;

/// Make the login item match `wanted`, changing nothing when it already does.
pub fn sync(app: &AppHandle, wanted: bool) -> Result<(), String> {
    let launcher = app.autolaunch();

    if launcher.is_enabled().map_err(|err| err.to_string())? == wanted {
        return Ok(());
    }

    if wanted {
        launcher.enable().map_err(|err| err.to_string())
    } else {
        launcher.disable().map_err(|err| err.to_string())
    }
}
