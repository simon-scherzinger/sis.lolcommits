//! Updates from the GitHub releases (latest.json, signed with the updater key).
//! Checks a minute after start and then every hour unless
//! `lolcommits.update.auto` is false. Installing waits for a running photo,
//! replaces the app and restarts it.
use crate::{config, tray};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub current: String,
    /// newer version on GitHub, if any
    pub available: Option<String>,
    pub notes: String,
    pub checking: bool,
    /// 0..=100 while downloading
    pub progress: Option<u8>,
    pub error: String,
    /// unix time of the last finished check
    pub checked: Option<i64>,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);

/// The updater from tauri.conf.json; LOLCOMMITS_UPDATE_URL points it at another
/// latest.json for tests.
fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let mut b = app.updater_builder();
    if let Ok(u) = std::env::var("LOLCOMMITS_UPDATE_URL") {
        b = b.endpoints(vec![u.parse().map_err(|e| format!("LOLCOMMITS_UPDATE_URL: {e}"))?]).map_err(|e| e.to_string())?;
    }
    b.build().map_err(|e| e.to_string())
}

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Status) -> T) -> T {
    let mut g = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(|| Status { current: app.package_info().version.to_string(), ..Default::default() });
    f(s)
}

pub fn status(app: &AppHandle) -> Status {
    with(app, |s| s.clone())
}

fn changed(app: &AppHandle) {
    let _ = app.emit("update-changed", status(app));
    tray::refresh(app);
}

pub fn auto_enabled() -> bool {
    config::get_or("update.auto", "true") != "false"
}

pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    if with(app, |s| s.checking || s.progress.is_some()) {
        return Ok(status(app).available);
    }
    with(app, |s| {
        s.checking = true;
        s.error.clear();
    });
    changed(app);
    let res = async { updater(app)?.check().await.map_err(|e| e.to_string()) }.await;
    let out = with(app, |s| {
        s.checking = false;
        s.checked = Some(config::now_unix());
        match &res {
            Ok(Some(u)) => {
                s.available = Some(u.version.clone());
                s.notes = u.body.clone().unwrap_or_default();
                Ok(s.available.clone())
            }
            Ok(None) => {
                s.available = None;
                s.notes.clear();
                Ok(None)
            }
            Err(e) => {
                s.error = e.clone();
                Err(e.clone())
            }
        }
    });
    changed(app);
    out
}

/// Download, install and restart. Waits until no photo is being taken.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if with(app, |s| s.progress.is_some()) {
        return Ok(());
    }
    let update = updater(app)?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else {
        with(app, |s| s.available = None);
        changed(app);
        return Err("lolcommits is up to date".into());
    };
    // the menu may still show an older version from the last check
    with(app, |s| {
        s.available = Some(update.version.clone());
        s.notes = update.body.clone().unwrap_or_default();
        s.progress = Some(0);
        s.error.clear();
    });
    changed(app);
    let mut done: u64 = 0;
    let mut last_pct = 0u8;
    let a = app.clone();
    let res = update
        .download(
            |chunk, total| {
                done += chunk as u64;
                if let Some(t) = total.filter(|t| *t > 0) {
                    let pct = ((done * 100 / t).min(100)) as u8;
                    if pct >= last_pct + 5 {
                        last_pct = pct;
                        with(&a, |s| s.progress = Some(pct));
                        changed(&a);
                    }
                }
            },
            || {},
        )
        .await;
    let bytes = match res {
        Ok(b) => b,
        Err(e) => {
            with(app, |s| {
                s.progress = None;
                s.error = e.to_string();
            });
            changed(app);
            return Err(e.to_string());
        }
    };
    while tray::ANIMATING.load(Ordering::SeqCst) {
        tokio_sleep(Duration::from_millis(300)).await;
    }
    if let Err(e) = update.install(bytes) {
        with(app, |s| {
            s.progress = None;
            s.error = e.to_string();
        });
        changed(app);
        return Err(e.to_string());
    }
    app.restart();
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

/// Background check: a minute after start, then every hour, so the menu
/// doesn't offer a version that a newer release has replaced.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let test = std::env::var("LOLCOMMITS_UPDATE_URL").is_ok();
        tokio_sleep(Duration::from_secs(if test { 3 } else { 60 })).await;
        loop {
            if auto_enabled() && (test || !cfg!(debug_assertions)) {
                if let Err(e) = check(&app).await {
                    crate::store::log(&format!("update check failed: {e}"));
                }
            }
            tokio_sleep(Duration::from_secs(3600)).await;
        }
    });
}
