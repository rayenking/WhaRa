//! Application wiring: plugins, window lifecycle, menu-bar bridge.

mod bridge;
mod config;
mod hotkeys;
mod notify;
mod refresh;
mod release;
mod shell;
mod startup;
mod tray;
mod updates;

use tauri::Manager as Registry;
use tauri_plugin_global_shortcut::ShortcutState;

/// Build and run the WhaRa application.
pub fn launch() {
    let shortcuts = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(fire_shortcut)
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_single_instance::init(reopen_app))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(shortcuts)
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(prepare)
        .on_window_event(watch_window)
        .invoke_handler(tauri::generate_handler![
            bridge::read_config,
            bridge::store_config,
            bridge::report_unread,
            bridge::flip_blur,
            bridge::apply_zoom,
            bridge::reveal_preferences,
            bridge::raise_chat,
            bridge::build_info,
            bridge::update_status,
            bridge::check_update,
            bridge::open_release,
            updates::updater_state,
            updates::updater_check_now,
            updates::updater_later,
            updates::updater_install,
            notify::push_notice,
        ])
        .build(tauri::generate_context!())
        .expect("cannot start WhaRa")
        .run(|_app, _event| {});
}

/// Re-open the chat window when a second instance is launched.
fn reopen_app(app: &tauri::AppHandle, _argv: Vec<String>, _cwd: String) {
    shell::resume_active_session(app);
}

fn prepare(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    config::bootstrap(&handle)?;
    shell::mount_chat(&handle)?;
    tray::register_tray(&handle)?;
    bridge::push_stored_state(&handle);

    refresh::bootstrap(&handle)?;
    updates::bootstrap(&handle);
    updates::schedule(handle.clone());

    let hotkey = config::snapshot(&handle).blur_hotkey;
    if let Err(reason) = hotkeys::assign(&handle, &hotkey) {
        eprintln!("[whara] hotkey {hotkey} unavailable: {reason}");
    }

    // Match the login item to the stored preference.
    let wanted = config::snapshot(&handle).launch_at_login;
    if let Err(reason) = startup::sync(&handle, wanted) {
        eprintln!("[whara] cannot set launch-at-login to {wanted}: {reason}");
    }

    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Regular);

    Ok(())
}

fn fire_shortcut(
    app: &tauri::AppHandle,
    _key: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state() == ShortcutState::Pressed {
        bridge::flip_blur(app.clone());
    }
}

fn watch_window(win: &tauri::Window, event: &tauri::WindowEvent) {
    let tauri::WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };

    if win.label() == updates::WINDOW {
        // Don't dismiss the progress UI in the middle of an installation.
        if updates::updater_installing(win.app_handle()) {
            api.prevent_close();
        } else if let Err(reason) = updates::remember_later(win.app_handle()) {
            eprintln!("[whara] cannot remember deferred update: {reason}");
        }
        return;
    }

    if win.label() == shell::CHAT && config::snapshot(win.app_handle()).keep_running_on_close {
        api.prevent_close();
        win.hide().ok();
    }
}
