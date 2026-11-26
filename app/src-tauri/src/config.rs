//! git config access – all settings live in `git config --global lolcommits.*`,
//! shared with the command line tool.
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Apps started from the Finder get almost no PATH; add the usual tool locations.
pub fn path_env() -> String {
    let mut parts = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{}/.local/bin", home().display()),
    ];
    parts.push(std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()));
    parts.join(":")
}

/// A command with the extended PATH, run from `/` so per-repo config never leaks in.
pub fn cmd(program: &str) -> Command {
    let mut c = Command::new(program);
    c.env("PATH", path_env()).current_dir("/");
    c
}

pub fn which(program: &str) -> Option<PathBuf> {
    path_env().split(':').map(|d| PathBuf::from(d).join(program)).find(|p| p.is_file())
}

fn git(args: &[&str]) -> Result<String, String> {
    let out = cmd("git").args(args).output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---- cached read of the global git config
// Every setting (and lol_dir(), behind nearly every file path) used to spawn
// `git config`, ~15 ms each and a dozen per window refresh. Now the whole
// global config is read once and again only when one of its files changed
// (size or modification time) or the app itself wrote to it. The command
// line tool writes the same files, so its changes are picked up too.

/// Files `git config --global` reads.
fn global_files() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("GIT_CONFIG_GLOBAL").filter(|p| !p.is_empty()) {
        return vec![PathBuf::from(p)];
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    vec![home().join(".gitconfig"), xdg.join("git").join("config")]
}

type Stamp = Vec<Option<(SystemTime, u64)>>;
type Entries = Vec<(String, String)>;
static CACHE: Mutex<Option<(Stamp, Entries)>> = Mutex::new(None);

fn stamp() -> Stamp {
    global_files().iter().map(|p| std::fs::metadata(p).ok().map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()))).collect()
}

/// All global entries in file order as (key, value); git lowercases section and key names.
fn entries() -> Entries {
    let st = stamp();
    if let Some((s, e)) = CACHE.lock().ok().as_deref().and_then(|c| c.as_ref()) {
        if *s == st {
            return e.clone();
        }
    }
    // -z: "key\nvalue\0" per entry, safe for values with spaces or newlines
    let e: Entries = git(&["config", "--global", "--list", "-z"])
        .map(|out| out.split('\0').filter(|x| !x.is_empty()).map(|x| {
            let (k, v) = x.split_once('\n').unwrap_or((x, ""));
            (k.to_string(), v.to_string())
        }).collect())
        .unwrap_or_default();
    if let Ok(mut c) = CACHE.lock() {
        *c = Some((st, e.clone()));
    }
    e
}

fn invalidate() {
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

/// Last value of a global key, like `git config --global --get`.
fn global_get(key: &str) -> Option<String> {
    let key = key.to_lowercase();
    entries().into_iter().rev().find(|(k, _)| k.to_lowercase() == key).map(|(_, v)| v)
}

/// All lolcommits.* keys, lower-cased the way git reports them (e.g. `pauseduntil`).
pub fn all() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for (k, v) in entries() {
        if let Some(rest) = k.strip_prefix("lolcommits.") {
            m.insert(rest.to_lowercase(), v);
        }
    }
    m
}

pub fn get(key: &str) -> Option<String> {
    all().remove(&key.to_lowercase()).filter(|v| !v.is_empty())
}

pub fn get_or(key: &str, default: &str) -> String {
    get(key).unwrap_or_else(|| default.to_string())
}

/// Set a key; an empty value removes it.
pub fn set(key: &str, value: &str) -> Result<(), String> {
    let k = format!("lolcommits.{key}");
    invalidate();
    if value.is_empty() {
        let _ = git(&["config", "--global", "--unset-all", &k]);
        Ok(())
    } else {
        git(&["config", "--global", &k, value]).map(|_| ())
    }
}

/// All values of a multi-valued key (e.g. lolcommits.folders).
pub fn get_all(key: &str) -> Vec<String> {
    let key = format!("lolcommits.{key}").to_lowercase();
    entries().into_iter().filter(|(k, v)| k.to_lowercase() == key && !v.is_empty()).map(|(_, v)| v).collect()
}

/// Replace all values of a multi-valued key.
pub fn set_all(key: &str, values: &[String]) -> Result<(), String> {
    let k = format!("lolcommits.{key}");
    invalidate();
    let _ = git(&["config", "--global", "--unset-all", &k]);
    for v in values {
        git(&["config", "--global", "--add", &k, v])?;
    }
    Ok(())
}

pub fn git_global(args: &[&str]) -> Result<String, String> {
    let mut a = vec!["config", "--global"];
    a.extend_from_slice(args);
    invalidate();
    git(&a)
}

pub fn git_version() -> String {
    // git is not replaced while the app runs: ask once
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| git(&["--version"]).unwrap_or_default().split_whitespace().nth(2).unwrap_or("").to_string()).clone()
}

/// git 2.54+ has config-based hooks, which `--global` needs.
pub fn has_config_hooks() -> bool {
    let v = git_version();
    let mut it = v.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let (a, b) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    (a, b) >= (2, 54)
}

pub fn hook_command() -> Option<String> {
    global_get("hook.lolcommits.command").filter(|s| !s.is_empty())
}

pub fn user_first_name() -> String {
    global_get("user.name").unwrap_or_default().split_whitespace().next().unwrap_or("").to_string()
}

/// Where pictures go: $LOLCOMMITS_DIR, lolcommits.dir, or ~/.lolcommits.
pub fn lol_dir() -> PathBuf {
    if let Ok(d) = std::env::var("LOLCOMMITS_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    match get("dir") {
        Some(d) if d.starts_with("~/") => home().join(&d[2..]),
        Some(d) => PathBuf::from(d),
        None => home().join(".lolcommits"),
    }
}

pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Paused either indefinitely (enabled=false) or until lolcommits.pausedUntil.
pub fn paused_until() -> Option<i64> {
    let c = all();
    if c.get("enabled").map(|v| v == "false").unwrap_or(false) {
        return Some(i64::MAX);
    }
    c.get("pauseduntil").and_then(|v| v.parse::<i64>().ok()).filter(|t| *t > now_unix())
}
