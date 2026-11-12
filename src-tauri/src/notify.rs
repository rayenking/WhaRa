//! Native notifications.
//!
//! WhatsApp Web asks WebKit for permission to show notifications, but WKWebView
//! has no consent UI on macOS and answers `denied` immediately. The page layer
//! therefore reports that notifications are granted and forwards the text here,
//! where it is posted through the real notification centre.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Post one notification. Failures are reported to the caller, which ignores
/// them: a chat must never break because the notification centre refused.
#[tauri::command]
pub fn push_notice(app: AppHandle, title: String, body: String) -> Result<(), String> {
    let trimmed_title = title.trim();
    let trimmed_body = body.trim();

    if trimmed_title.is_empty() && trimmed_body.is_empty() {
        return Ok(());
    }

    let mut builder = app
        .notification()
        .builder()
        .title(if trimmed_title.is_empty() {
            "WhaRa"
        } else {
            trimmed_title
        });

    if !trimmed_body.is_empty() {
        builder = builder.body(trimmed_body);
    }

    builder.show().map_err(|reason| reason.to_string())
}
