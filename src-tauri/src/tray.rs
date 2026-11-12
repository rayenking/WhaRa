//! Menu-bar glyph, its contextual menu and the unread counter.

use tauri::image::Image as Glyph;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;
use tauri::Manager as AppRegistry;

use crate::{config, shell};

const TRAY_HANDLE: &str = "whara-menubar";

/// `false` kept as a named constant so the builder call reads clearly.
const BOOL_NO: bool = false;

/// Attach the menu-bar icon and wire its menu actions.
pub fn register_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open WhaRa", true, Some("CmdOrCtrl+O"))?;
    let stow = MenuItem::with_id(app, "stow", "Hide Window", true, None::<&str>)?;
    let veil = MenuItem::with_id(
        app,
        "veil",
        "Toggle Privacy Blur",
        true,
        Some("CmdOrCtrl+Shift+B"),
    )?;
    let prefs = MenuItem::with_id(app, "prefs", "Preferences…", true, Some("CmdOrCtrl+,"))?;
    let divider = PredefinedMenuItem::separator(app)?;
    let stop = MenuItem::with_id(app, "stop", "Quit WhaRa", true, Some("CmdOrCtrl+Q"))?;

    let entries = Menu::with_items(app, &[&open, &stow, &veil, &prefs, &divider, &stop])?;

    let glyph = Glyph::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id(TRAY_HANDLE)
        .icon(glyph)
        .icon_as_template(true)
        .tooltip("WhaRa")
        .menu(&entries)
        .show_menu_on_left_click(BOOL_NO)
        .on_menu_event(|app, event| route(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| react_to_click(tray.app_handle(), event))
        .build(app)?;

    Ok(())
}

fn route(app: &AppHandle, action: &str) {
    let target = match action {
        "open" => shell::resume_active_session,
        "stow" => shell::minimise_to_menubar,
        "veil" => shell::flip_privacy_blur,
        "prefs" => shell::open_preferences,
        "stop" => return app.exit(0),
        _ => return,
    };
    target(app);
}

/// Only a plain left click toggles the chat panel.
fn react_to_click(app: &AppHandle, event: TrayIconEvent) {
    // Only a plain left click toggles the panel; right click opens the menu.
    if !matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    ) {
        return;
    }

    shell::toggle_chat(app);
}

/// Mirror the unread count into the menu-bar title and the chat heading.
pub fn display_unread(app: &AppHandle, total: u32) {
    let prefs = config::snapshot(app);

    if let Some(tray) = app.tray_by_id(TRAY_HANDLE) {
        let counter = if prefs.tray_counter && total > 0 {
            Some(if total > 99 {
                "99+".into()
            } else {
                total.to_string()
            })
        } else {
            None
        };
        tray.set_title(counter.as_deref()).ok();

        let tip = if total > 0 {
            format!("WhaRa — {total} unread")
        } else {
            "WhaRa".into()
        };
        tray.set_tooltip(Some(tip.as_str())).ok();
    }

    if let Some(panel) = app.get_webview_window(shell::CHAT) {
        let heading = if total > 0 {
            format!("WhaRa ({total})")
        } else {
            "WhaRa".into()
        };
        panel.set_title(heading.as_str()).ok();
    }
}
