//! Legacy GitHub release-feed metadata for the Preferences release link.
//! Signed in-app installation and periodic checks live in `updates.rs`.
//! The WhatsApp page is not granted access to either updater workflow.

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::release;

/// Cache duration used by the legacy on-demand release-feed lookup.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// What the preferences page shows about updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// Version of the running build.
    pub current: String,
    /// Newest published version, when a check has succeeded.
    pub latest: Option<String>,
    /// Release page for `latest`.
    pub page: Option<String>,
    /// Whether `latest` is newer than `current`.
    pub available: bool,
    /// Last successful check, as milliseconds since the epoch.
    pub checked_at: Option<u64>,
    /// Failure text from the most recent attempt.
    pub error: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            current: release::VERSION.to_string(),
            latest: None,
            page: None,
            available: false,
            checked_at: None,
            error: None,
        }
    }
}

/// Bookkeeping kept beside `preferences.json`, written separately so a settings
/// save from the interface can never clobber it.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
struct History {
    last_check_ms: Option<u64>,
    /// Newest version the user has already been notified about.
    notified: Option<String>,
}

pub struct UpdateState(Mutex<Status>);

#[derive(Default)]
struct Status {
    history: History,
    status: UpdateStatus,
}

fn history_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|folder| folder.join("update-history.json"))
}

fn load_history(app: &AppHandle) -> History {
    history_file(app)
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|raw| serde_json::from_str::<History>(&raw).ok())
        .unwrap_or_default()
}

fn save_history(app: &AppHandle, history: &History) {
    let Some(file) = history_file(app) else {
        return;
    };
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(raw) = serde_json::to_string_pretty(history) {
        std::fs::write(file, raw).ok();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|span| span.as_millis() as u64)
        .unwrap_or(0)
}

pub fn bootstrap(app: &AppHandle) -> tauri::Result<()> {
    let history = load_history(app);
    let status = UpdateStatus {
        current: release::VERSION.to_string(),
        checked_at: history.last_check_ms,
        ..UpdateStatus::default()
    };
    app.manage(UpdateState(Mutex::new(Status { history, status })));
    Ok(())
}

/// Publish a status snapshot to the preferences window, if it is open.
fn broadcast(app: &AppHandle, status: &UpdateStatus) {
    if let Some(panel) = app.get_webview_window(crate::shell::PREFS) {
        panel.emit("update-status", status.clone()).ok();
    }
}

/// Current status, as shown in the preferences page.
pub fn status(app: &AppHandle) -> UpdateStatus {
    match app.try_state::<UpdateState>() {
        Some(state) => match state.0.lock() {
            Ok(guard) => guard.status.clone(),
            Err(_) => UpdateStatus::default(),
        },
        None => UpdateStatus::default(),
    }
}

/// Ask GitHub for the newest release and record the result.
///
/// `force` skips the "already notified about this version" short-circuit so a
/// manual check always hits the network.
pub fn check(app: &AppHandle, force: bool) -> Result<UpdateStatus, String> {
    // Without a configured repository there is nothing to look up, and saying
    // "checking" forever would be a lie.
    if release::REPOSITORY.is_none() {
        let mut idle = status(app);
        idle.error =
            Some("Update checking is not configured for this build (set WHARA_REPOSITORY).".into());
        return Ok(idle);
    }

    let (already_notified, last_check) = match app.try_state::<UpdateState>() {
        Some(state) => match state.0.lock() {
            Ok(guard) => (guard.history.notified.clone(), guard.history.last_check_ms),
            Err(_) => (None, None),
        },
        None => (None, None),
    };

    // Quiet background checks skip work when nothing can have changed.
    if !force {
        if let Some(stamp) = last_check {
            let age = now_ms().saturating_sub(stamp);
            if age < CHECK_INTERVAL.as_millis() as u64 {
                return Ok(status(app));
            }
        }
    }

    let outcome = fetch_latest();
    let mut snapshot = status(app);

    match outcome {
        Ok((tag, page)) => {
            let available = release::is_newer(&tag, &snapshot.current);

            // Only surface a version we have not already reported.
            let announce =
                available && (!force || already_notified.as_deref() != Some(tag.as_str()));
            if !force && already_notified.as_deref() == Some(tag.as_str()) {
                snapshot.available = true;
                snapshot.latest = Some(tag);
                snapshot.page = Some(page);
                return Ok(snapshot);
            }
            let _ = announce;

            snapshot.latest = Some(tag.clone());
            snapshot.page = Some(page);
            snapshot.available = available;
            snapshot.checked_at = Some(now_ms());
            snapshot.error = None;

            if let Some(state) = app.try_state::<UpdateState>() {
                if let Ok(mut guard) = state.0.lock() {
                    guard.history.last_check_ms = snapshot.checked_at;
                    if available {
                        guard.history.notified = Some(tag);
                    }
                    guard.status = snapshot.clone();
                    let history = guard.history.clone();
                    drop(guard);
                    save_history(app, &history);
                }
            }

            broadcast(app, &snapshot);
            Ok(snapshot)
        }
        Err(reason) => {
            let mut snapshot = status(app);
            snapshot.error = Some(reason.clone());
            snapshot.checked_at = Some(now_ms());

            if let Some(state) = app.try_state::<UpdateState>() {
                if let Ok(mut guard) = state.0.lock() {
                    guard.history.last_check_ms = snapshot.checked_at;
                    guard.status = snapshot.clone();
                    let history = guard.history.clone();
                    drop(guard);
                    save_history(app, &history);
                }
            }

            broadcast(app, &snapshot);
            Err(reason)
        }
    }
}

/// Release feed lookup. Returns the tag and the human-facing release page.
fn fetch_latest() -> Result<(String, String), String> {
    let repository = release::REPOSITORY.ok_or("no release repository configured")?;
    let url = format!("https://api.github.com/repos/{repository}/releases/latest");

    let body = ureq::get(&url)
        .header("User-Agent", concat!("WhaRa/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| err.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|err| err.to_string())?;

    let payload: serde_json::Value =
        serde_json::from_str(&body).map_err(|err| format!("unreadable release feed: {err}"))?;

    let tag = payload
        .get("tag_name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "release feed has no tag_name".to_string())?
        .to_string();

    let page = payload
        .get("html_url")
        .and_then(|value| value.as_str())
        .unwrap_or("https://github.com")
        .to_string();

    Ok((tag, page))
}

/// Hand the release page to the default browser.
pub fn open_page(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("refusing to open a non-https link".into());
    }
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}
