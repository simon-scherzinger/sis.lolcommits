//! Team settings in the shared drive, one choice for everyone:
//!   .meta/repos/<sha256 of the repo name>   "show\t<name>" or "hide"
//!   .meta/retention                          days: the longest anyone keeps shared pictures
//! One small file per setting, so two people writing at once never edit the
//! same file (no Drive conflict copies). A hidden repo appears only as a hash,
//! so the settings don't put a name in the drive that the team keeps hidden.
//! Read from the folder itself, or for rclone from the local mirror (kept
//! fresh by refresh() and the team sync); uploads read the drive directly.
use crate::share::{self, Target, META};
use crate::{config, store};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn hash(repo: &str) -> String {
    Sha256::digest(repo.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn repo_rel(repo: &str) -> String {
    format!("{META}/repos/{}", hash(repo))
}

fn retention_rel() -> String {
    format!("{META}/retention")
}

/// The file as it is on this computer: in the folder, or in the rclone mirror.
fn local(rel: &str) -> Option<PathBuf> {
    share::team_dir().ok().map(|d| d.join(rel))
}

fn read_local(rel: &str) -> Option<String> {
    fs::read_to_string(local(rel)?).ok()
}

/// Straight from the drive (rclone; a folder is read directly anyway).
/// Ok(None): there is no such file.
fn read_fresh(rel: &str) -> Result<Option<String>, String> {
    match share::target()? {
        Target::Folder(_) => Ok(read_local(rel)),
        Target::Rclone(base) => match share::rclone(&["cat", &format!("{base}/{rel}")]) {
            Ok(s) => {
                mirror(rel, Some(&s));
                Ok(Some(s))
            }
            Err(e) if e.contains("not found") => {
                mirror(rel, None);
                Ok(None)
            }
            Err(e) => Err(e),
        },
    }
}

/// Keep the rclone mirror in step with what we read or wrote.
fn mirror(rel: &str, content: Option<&str>) {
    if !matches!(share::target(), Ok(Target::Rclone(_))) {
        return;
    }
    let Some(p) = local(rel) else { return };
    match content {
        Some(c) => {
            let _ = fs::create_dir_all(p.parent().unwrap());
            let _ = fs::write(&p, c);
        }
        None => {
            let _ = fs::remove_file(&p);
        }
    }
}

fn write(rel: &str, content: Option<&str>) -> Result<(), String> {
    let t = share::target()?;
    match content {
        Some(c) => {
            let tmp = share::tmp_file(&format!("team-{}", hash(rel)));
            fs::write(&tmp, c).map_err(|e| e.to_string())?;
            let res = share::put(&t, &tmp, rel);
            let _ = fs::remove_file(&tmp);
            res?
        }
        None => share::rm(&t, rel)?,
    }
    mirror(rel, content);
    Ok(())
}

fn parse_choice(s: &str) -> Option<bool> {
    match s.split('\t').next()?.trim() {
        "show" => Some(true),
        "hide" => Some(false),
        _ => None,
    }
}

/// The team's choice for a repo as known on this computer. Two people
/// answering at once can leave a Drive conflict copy ("<hash> (1)") next to
/// the file: then hide wins.
pub fn repo_choice(repo: &str) -> Option<bool> {
    let dir = local(&format!("{META}/repos"))?;
    let h = hash(repo);
    let choices: Vec<bool> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&h))
        .filter_map(|e| fs::read_to_string(e.path()).ok().as_deref().and_then(parse_choice))
        .collect();
    if choices.is_empty() { None } else { Some(choices.iter().all(|c| *c)) }
}

/// The team's choice straight from the drive.
pub fn fresh_repo_choice(repo: &str) -> Result<Option<bool>, String> {
    match share::target()? {
        Target::Folder(_) => Ok(repo_choice(repo)),
        Target::Rclone(_) => Ok(read_fresh(&repo_rel(repo))?.as_deref().and_then(parse_choice)),
    }
}

pub fn set_repo_choice(repo: &str, show: bool) -> Result<(), String> {
    let content = if show { format!("show\t{}\n", repo.replace(['\t', '\n'], " ")) } else { "hide\n".into() };
    write(&repo_rel(repo), Some(&content))?;
    // a deliberate choice replaces conflict copies ("<hash> (1)"), which would otherwise keep winning
    if let Ok(Target::Folder(_)) = share::target() {
        let h = hash(repo);
        if let Some(dir) = local(&format!("{META}/repos")) {
            for e in fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&h) && name != h {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }
    Ok(())
}

/// Team maximum for keeping shared pictures, in days.
pub fn retention() -> Option<i64> {
    read_local(&retention_rel())?.trim().parse().ok().filter(|d| *d > 0)
}

pub fn set_retention(days: Option<i64>) -> Result<(), String> {
    write(&retention_rel(), days.filter(|d| *d > 0).map(|d| format!("{d}\n")).as_deref())
}

/// rclone: fetch the team settings into the mirror (one call). Folders are read directly.
pub fn refresh() -> Result<(), String> {
    if let Target::Rclone(base) = share::target()? {
        let dir = share::team_dir()?;
        let from = format!("{base}/{META}");
        let to = dir.join(META);
        fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        match share::rclone(&["sync", &from, to.to_str().unwrap(), "--include", "/repos/**", "--include", "/retention"]) {
            Err(e) if !e.contains("not found") => return Err(e),
            _ => {} // no .meta/ yet: no team settings
        }
    }
    Ok(())
}

/// Once per team gallery: publish this computer's repo choices from before
/// team settings existed. Where the team has a value, the stricter one wins:
/// hidden stays hidden. Returns how many were written.
pub fn publish_local_choices() -> Result<usize, String> {
    let target = config::get("share.target").unwrap_or_default();
    let marker = config::lol_dir().join(".team-published");
    if fs::read_to_string(&marker).ok().as_deref() == Some(target.as_str()) {
        return Ok(0);
    }
    let mut n = 0;
    for (repo, show) in share::local_repo_choices() {
        let team = fresh_repo_choice(&repo)?;
        let want = team.map_or(show, |t| t && show);
        if team != Some(want) {
            set_repo_choice(&repo, want)?;
            n += 1;
        }
    }
    let _ = fs::write(&marker, &target);
    if n > 0 {
        store::log(&format!("published {n} repo name choices to the team"));
    }
    Ok(n)
}
