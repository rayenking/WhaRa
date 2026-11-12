//! Lifecycle of the global privacy-veil shortcut.
//!
//! The binding lives in the preferences file, so it has to be re-applied
//! whenever that file changes — otherwise the old combination stays live until
//! the next launch.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt as ShortcutDesk, Shortcut};

/// Turn a stored string into a shortcut, or `None` when it is blank or invalid.
///
/// Kept separate from the registration so the parsing rules are testable
/// without an application instance.
pub fn parse(key: &str) -> Option<Shortcut> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<Shortcut>().ok()
}

/// Bind `key`, dropping whatever was registered before.
///
/// A blank or unparseable value clears the binding: no shortcut fires, which is
/// the honest result of an unusable combination.
pub fn assign(app: &AppHandle, key: &str) -> Result<(), String> {
    let desk = app.global_shortcut();
    desk.unregister_all().map_err(|err| err.to_string())?;

    match parse(key) {
        Some(shortcut) => desk.register(shortcut).map_err(|err| err.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The binding shown in Preferences must survive parsing.
    #[test]
    fn default_binding_parses() {
        assert!(parse("CmdOrCtrl+Shift+B").is_some());
    }

    /// Blank input clears rather than registering something odd.
    #[test]
    fn blank_input_clears() {
        assert!(parse("").is_none());
        assert!(parse("   ").is_none());
    }

    /// Nonsense is rejected instead of panicking.
    #[test]
    fn nonsense_is_rejected() {
        assert!(parse("Definitely+Not+A+Key").is_none());
    }
}
