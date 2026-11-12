//! Stored preferences (JSON under the macOS application-support folder).

use std::path::PathBuf as StoragePath;
use std::sync::Mutex as Guard;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Manager as Registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    /// Interface scale, clamped to 0.6 ..= 2.0
    pub zoom: f64,
    /// Hide message contents until the pointer enters the window
    pub privacy_blur: bool,
    /// System-wide shortcut toggling the privacy blur
    pub blur_hotkey: String,
    /// Start together with the user session
    pub launch_at_login: bool,
    /// Draw the unread counter beside the menu-bar glyph
    pub tray_counter: bool,
    /// Hide instead of quitting when the red button is pressed
    pub keep_running_on_close: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            privacy_blur: false,
            blur_hotkey: String::from("CmdOrCtrl+Shift+B"),
            launch_at_login: false,
            tray_counter: true,
            keep_running_on_close: true,
        }
    }
}

impl Preferences {
    /// Clamp values that must stay inside a sane range.
    pub fn tame(mut self) -> Self {
        self.zoom = self.zoom.clamp(0.6, 2.0);
        if self.blur_hotkey.trim().is_empty() {
            self.blur_hotkey = Preferences::default().blur_hotkey;
        }
        self
    }
}

pub struct Store(pub Guard<Preferences>);

fn store_file(app: &AppHandle) -> Option<StoragePath> {
    let folder = app.path().app_config_dir().ok()?;
    Some(folder.join("preferences.json"))
}

pub fn read(app: &AppHandle) -> Preferences {
    let raw = store_file(app).and_then(|file| std::fs::read_to_string(file).ok());
    match raw.and_then(|text| serde_json::from_str::<Preferences>(&text).ok()) {
        Some(parsed) => parsed.tame(),
        None => Preferences::default(),
    }
}

pub fn write(app: &AppHandle, value: &Preferences) -> Result<(), String> {
    let file = store_file(app).ok_or("missing config directory")?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let payload = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(file, payload).map_err(|err| err.to_string())
}

pub fn bootstrap(app: &AppHandle) -> tauri::Result<()> {
    app.manage(Store(Guard::new(read(app))));
    Ok(())
}

/// Current preferences, cheap to clone.
pub fn snapshot(app: &AppHandle) -> Preferences {
    match app.try_state::<Store>() {
        Some(state) => match state.0.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => Preferences::default(),
        },
        None => Preferences::default(),
    }
}

pub fn store(app: &AppHandle, value: Preferences) -> Result<Preferences, String> {
    let value = value.tame();
    if let Some(state) = app.try_state::<Store>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = value.clone();
        }
    }
    write(app, &value)?;
    Ok(value)
}
