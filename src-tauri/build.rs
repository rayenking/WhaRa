//! Build script.
//!
//! Besides Tauri's usual code generation, this registers the application's own
//! commands with the ACL. Without that, Tauri generates no `__app-acl__`
//! manifest, and any IPC from a remote origin (the WhatsApp Web page) is
//! rejected with "not allowed by ACL" — which is exactly how the page layer
//! lost its bridge to the host.

/// Commands the page layer and the preferences window may call.
const COMMANDS: &[&str] = &[
    "read_config",
    "store_config",
    "report_unread",
    "flip_blur",
    "apply_zoom",
    "reveal_preferences",
    "raise_chat",
    "build_info",
    "update_status",
    "check_update",
    "open_release",
    "updater_state",
    "updater_check_now",
    "updater_later",
    "updater_install",
    "push_notice",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
