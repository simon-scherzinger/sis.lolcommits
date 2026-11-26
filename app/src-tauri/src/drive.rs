//! Google Drive setup without a terminal:
//! 1. Google Drive for desktop: shared drives are plain folders under
//!    ~/Library/CloudStorage/GoogleDrive-<account>/<Shared drives>/ – no login needed.
//! 2. Otherwise rclone: browser login, then pick a shared drive from a list.
use crate::config;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// "Shared drives" in the languages Drive for desktop uses most.
const SHARED_NAMES: [&str; 9] = [
    "Shared drives", "Geteilte Ablagen", "Drive partagés", "Unidades compartidas", "Drive condivisi",
    "Gedeelde drives", "Drives compartilhados", "Udostępnione dyski", "Delade enheter",
];
const MY_NAMES: [&str; 8] = ["My Drive", "Meine Ablage", "Mon Drive", "Mi unidad", "Il mio Drive", "Mijn Drive", "Meu Drive", "Other computers"];

#[derive(Serialize)]
pub struct DriveFolder {
    pub account: String,
    pub name: String,
    pub path: String,
}

fn cloud_storage() -> PathBuf {
    std::env::var_os("LOLCOMMITS_CLOUDSTORAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join("Library/CloudStorage"))
}

fn dirs(p: &PathBuf) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
                .filter(|(n, _)| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Shared drives that Google Drive for desktop has mounted.
pub fn folders() -> Vec<DriveFolder> {
    let mut out = Vec::new();
    for (acct_dir, acct_path) in dirs(&cloud_storage()) {
        let Some(account) = acct_dir.strip_prefix("GoogleDrive-") else { continue };
        let top = dirs(&acct_path);
        let containers: Vec<PathBuf> = {
            let known: Vec<PathBuf> = top.iter().filter(|(n, _)| SHARED_NAMES.contains(&n.as_str())).map(|(_, p)| p.clone()).collect();
            if !known.is_empty() {
                known
            } else {
                // unknown language: anything that isn't "My Drive" or a shortcut store
                top.iter()
                    .filter(|(n, _)| !MY_NAMES.contains(&n.as_str()) && !n.contains("shortcut-targets"))
                    .map(|(_, p)| p.clone())
                    .collect()
            }
        };
        for c in containers {
            for (name, path) in dirs(&c) {
                out.push(DriveFolder { account: account.to_string(), name, path: path.to_string_lossy().to_string() });
            }
        }
    }
    out
}

pub const REMOTE: &str = "lolcommits-gdrive";

pub fn rclone_installed() -> bool {
    config::which("rclone").is_some()
}

fn rclone(args: &[&str]) -> Result<String, String> {
    let bin = config::which("rclone").ok_or("rclone is not installed")?;
    let out = config::cmd(bin.to_str().unwrap()).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("rclone failed").to_string())
    }
}

pub fn remote_exists() -> bool {
    rclone(&["listremotes"]).map(|o| o.lines().any(|l| l.trim() == format!("{REMOTE}:"))).unwrap_or(false)
}

#[derive(Serialize, Deserialize)]
pub struct SharedDrive {
    pub id: String,
    pub name: String,
}

/// The running browser sign-in, so the UI can cancel it.
static LOGIN: Mutex<Option<Child>> = Mutex::new(None);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The Google OAuth client ("Desktop app") rclone signs in with:
/// 1. the team's own, entered in the setup (lolcommits.share.googleClientId/-Secret)
/// 2. LOLCOMMITS_GDRIVE_CLIENT_ID/_SECRET at runtime (tests)
/// 3. the one built into this build (CI secrets; entrecode's, "Internal")
/// rclone's shared client is not used: Google retired it in 2026.
pub fn client() -> Option<(String, String)> {
    if let Some(id) = config::get("share.googleClientId") {
        return Some((id, config::get("share.googleClientSecret").unwrap_or_default()));
    }
    let var = |k: &str, built: Option<&str>| std::env::var(k).ok().or(built.map(String::from)).filter(|v| !v.is_empty());
    let id = var("LOLCOMMITS_GDRIVE_CLIENT_ID", option_env!("LOLCOMMITS_GDRIVE_CLIENT_ID"))?;
    Some((id, var("LOLCOMMITS_GDRIVE_CLIENT_SECRET", option_env!("LOLCOMMITS_GDRIVE_CLIENT_SECRET")).unwrap_or_default()))
}

/// "own", "builtin" or "none" – for the setup screen.
pub fn client_source() -> &'static str {
    if config::get("share.googleClientId").is_some() {
        "own"
    } else if client().is_some() {
        "builtin"
    } else {
        "none"
    }
}

const NO_CLIENT: &str = "Google sign-in needs a Google client ID for your team – or use Google Drive for desktop.";

/// Signed in with the client lolcommits uses now (so no browser is needed).
pub fn signed_in() -> bool {
    remote_exists() && client().map(|(id, _)| id == remote_client_id()).unwrap_or(false)
}

fn remote_client_id() -> String {
    rclone(&["config", "show", REMOTE])
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("client_id = ").map(|v| v.trim().to_string()))
        .unwrap_or_default()
}

/// Shared drives of the signed-in account. Reuses an existing sign-in and only
/// opens the browser when there is none, Google no longer accepts it, or it was
/// made with another client (e.g. rclone's retired shared one).
pub fn connect() -> Result<Vec<SharedDrive>, String> {
    let (id, _) = client().ok_or(NO_CLIENT)?;
    if remote_exists() {
        if remote_client_id() == id {
            match shared_drives() {
                Ok(d) => return Ok(d),
                Err(e) if !auth_error(&e) => return Err(e),
                Err(_) => {}
            }
        }
        disconnect();
    }
    login()?;
    shared_drives()
}

fn auth_error(e: &str) -> bool {
    let e = e.to_lowercase();
    ["token", "oauth", "invalid_grant", "unauthorized", "401", "403"].iter().any(|k| e.contains(k))
}

/// The page Google's redirect lands on after the sign-in.
const AUTH_PAGE: &str = include_str!("auth-page.html");

/// Browser sign-in with `rclone authorize` (our own success page), then the
/// remote is created from the token – without asking anything else.
fn login() -> Result<(), String> {
    let bin = config::which("rclone").ok_or("rclone is not installed")?;
    let page = std::env::temp_dir().join("lolcommits-auth-page.html");
    fs::write(&page, AUTH_PAGE).map_err(|e| e.to_string())?;
    let (id, secret) = client().ok_or(NO_CLIENT)?;
    let args = ["authorize".to_string(), "drive".into(), id.clone(), secret.clone(), "--template".into(), page.to_string_lossy().to_string()];
    let child = config::cmd(bin.to_str().unwrap())
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    *LOGIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
    let start = Instant::now();
    let (ok, out, err) = loop {
        std::thread::sleep(Duration::from_millis(300));
        let mut g = LOGIN.lock().unwrap_or_else(|e| e.into_inner());
        let Some(c) = g.as_mut() else { return Err("Sign-in cancelled".into()) };
        match c.try_wait() {
            Ok(Some(st)) => {
                let mut c = g.take().unwrap();
                let (mut out, mut err) = (String::new(), String::new());
                if let Some(mut o) = c.stdout.take() {
                    let _ = o.read_to_string(&mut out);
                }
                if let Some(mut e) = c.stderr.take() {
                    let _ = e.read_to_string(&mut err);
                }
                break (st.success(), out, err);
            }
            Ok(None) if start.elapsed() > LOGIN_TIMEOUT => {
                let _ = c.kill();
                let _ = c.wait();
                g.take();
                return Err("Sign-in timed out. Try again.".into());
            }
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = fs::remove_file(&page);
    let token = token_from(&out).filter(|_| ok).ok_or_else(|| {
        err.lines().filter(|l| !l.trim().is_empty() && !l.contains("NOTICE")).last().unwrap_or("Sign-in failed").to_string()
    })?;
    let create = [
        "config".to_string(), "create".into(), REMOTE.into(), "drive".into(), "scope=drive".into(), format!("token={token}"),
        format!("client_id={id}"), format!("client_secret={secret}"),
        "config_refresh_token=false".into(), "config_change_team_drive=false".into(), "--non-interactive".into(),
    ];
    let refs: Vec<&str> = create.iter().map(String::as_str).collect();
    let res = rclone(&refs)?;
    match serde_json::from_str::<serde_json::Value>(&res).ok().and_then(|v| v["Error"].as_str().map(String::from)) {
        Some(e) if !e.is_empty() => {
            disconnect();
            Err(e)
        }
        _ => Ok(()),
    }
}

/// The token JSON from `rclone authorize` output ("--->" … "<---End paste").
pub fn token_from(out: &str) -> Option<String> {
    let (a, b) = (out.find('{')?, out.rfind('}')?);
    let t = out.get(a..=b)?;
    serde_json::from_str::<serde_json::Value>(t).ok().filter(|v| v.get("access_token").is_some()).map(|_| t.to_string())
}

/// Stop a running browser sign-in (the "Cancel" button).
pub fn cancel_login() {
    if let Some(mut c) = LOGIN.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

pub fn shared_drives() -> Result<Vec<SharedDrive>, String> {
    let out = rclone(&["backend", "drives", &format!("{REMOTE}:")])?;
    parse_drives(&out)
}

pub fn parse_drives(json: &str) -> Result<Vec<SharedDrive>, String> {
    #[derive(Deserialize)]
    struct Raw {
        id: String,
        name: String,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| format!("unexpected rclone output: {e}"))?;
    Ok(raw.into_iter().map(|r| SharedDrive { id: r.id, name: r.name }).collect())
}

/// The share target for one shared drive. The drive goes into the target
/// itself (rclone connection string), so switching drives never touches the
/// remote – and never asks Google again.
pub fn target_for(id: &str) -> String {
    format!("rclone:{REMOTE},team_drive={id}:lolcommits")
}

pub fn disconnect() {
    let _ = rclone(&["config", "delete", REMOTE]);
}
