//! git config access – all settings live in `git config --global lolcommits.*`,
//! shared with the command line tool.
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

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

/// All lolcommits.* keys, lower-cased the way git reports them (e.g. `pauseduntil`).
pub fn all() -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(out) = git(&["config", "--global", "--get-regexp", r"^lolcommits\."]) {
        for line in out.lines() {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            m.insert(k.trim_start_matches("lolcommits.").to_lowercase(), v.to_string());
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
    if value.is_empty() {
        let _ = git(&["config", "--global", "--unset-all", &k]);
        Ok(())
    } else {
        git(&["config", "--global", &k, value]).map(|_| ())
    }
}

/// All values of a multi-valued key (e.g. lolcommits.folders).
pub fn get_all(key: &str) -> Vec<String> {
    git(&["config", "--global", "--get-all", &format!("lolcommits.{key}")])
        .map(|o| o.lines().filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// Replace all values of a multi-valued key.
pub fn set_all(key: &str, values: &[String]) -> Result<(), String> {
    let k = format!("lolcommits.{key}");
    let _ = git(&["config", "--global", "--unset-all", &k]);
    for v in values {
        git(&["config", "--global", "--add", &k, v])?;
    }
    Ok(())
}

pub fn git_global(args: &[&str]) -> Result<String, String> {
    let mut a = vec!["config", "--global"];
    a.extend_from_slice(args);
    git(&a)
}

pub fn git_version() -> String {
    git(&["--version"]).unwrap_or_default().split_whitespace().nth(2).unwrap_or("").to_string()
}

/// git 2.54+ has config-based hooks, which `--global` needs.
pub fn has_config_hooks() -> bool {
    let v = git_version();
    let mut it = v.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let (a, b) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    (a, b) >= (2, 54)
}

pub fn hook_command() -> Option<String> {
    git_global(&["--get", "hook.lolcommits.command"]).ok().filter(|s| !s.is_empty())
}

pub fn user_first_name() -> String {
    git_global(&["--get", "user.name"]).unwrap_or_default().split_whitespace().next().unwrap_or("").to_string()
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
