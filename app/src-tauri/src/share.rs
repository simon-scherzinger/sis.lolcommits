//! The team gallery, sorted by repo:
//!   <target>/<repo>/<YYYYmmdd>-<person>-<rand>.jpg
//!   <target>/<repo>/<YYYYmmdd>-<person>-<rand>.tsv   (date, name, repo or "-", subject)
//! Repos the person didn't allow by name go to <target>/other-repos/ with the
//! repo label blurred. One sidecar per picture, so people sharing into the same
//! folder at the same time never edit the same file (no Drive conflict copies).
//! Targets: a folder (Google Drive for desktop, NAS) or an rclone remote.
use crate::store::{self, Shot};
use crate::{config, render};
use rand::Rng;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const RCLONE_FLAGS: [&str; 1] = ["--drive-use-trash=false"];
pub const OTHER: &str = "other-repos";

#[derive(Debug, Clone)]
pub enum Target {
    Folder(PathBuf),
    Rclone(String),
}

pub fn target() -> Result<Target, String> {
    let t = config::get("share.target").ok_or("sharing is not set up")?;
    if let Some(r) = t.strip_prefix("rclone:") {
        return Ok(Target::Rclone(r.to_string()));
    }
    let expanded = if let Some(rest) = t.strip_prefix("~/") { config::home().join(rest) } else { PathBuf::from(&t) };
    if t.contains(':') && !t.starts_with('/') && !t.starts_with('~') {
        return Err(format!("{t} is an ssh target – those work with the lolcommits command line tool only"));
    }
    Ok(Target::Folder(expanded))
}

pub fn configured() -> bool {
    target().is_ok() && config::get("share.name").is_some()
}

/// Person part of file names, the same as slug() in the CLI.
pub fn slug(name: &str) -> String {
    let mut s = name.to_string();
    for (a, b) in [("ä", "ae"), ("Ä", "ae"), ("ö", "oe"), ("Ö", "oe"), ("ü", "ue"), ("Ü", "ue"), ("ß", "ss")] {
        s = s.replace(a, b);
    }
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Folder name for a repo: keeps case, dots and dashes; never hidden or empty.
pub fn repo_folder(repo: &str) -> String {
    let s: String = repo.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '-' }).collect();
    let s = s.trim_start_matches('.').to_string();
    if s.is_empty() || s == OTHER { "repo".into() } else { s }
}

fn me() -> Result<(String, String), String> {
    let name = config::get("share.name").ok_or("set your name first")?;
    let s = slug(&name);
    if s.is_empty() {
        return Err("your name needs at least one letter or digit".into());
    }
    Ok((name, s))
}

// ------------------------------------------------------------ per-repo choice
// ~/.lolcommits/repo-names.tsv: repo \t show|hide – asked once per repo.

fn names_path() -> PathBuf {
    config::lol_dir().join("repo-names.tsv")
}

/// Some(true) = show by name, Some(false) = keep hidden, None = not asked yet
pub fn repo_choice(repo: &str) -> Option<bool> {
    store::tsv(&names_path(), 2).into_iter().find(|r| r[0] == repo).map(|r| r[1] == "show")
}

pub fn repo_choices() -> Vec<(String, bool)> {
    store::tsv(&names_path(), 2).into_iter().map(|r| (r[0].clone(), r[1] == "show")).collect()
}

pub fn set_repo_choice(repo: &str, show: bool) -> Result<(), String> {
    let mut rows: Vec<String> = store::tsv(&names_path(), 2)
        .into_iter()
        .filter(|r| r[0] != repo)
        .map(|r| format!("{}\t{}\n", r[0], r[1]))
        .collect();
    rows.push(format!("{}\t{}\n", repo.replace(['\t', '\n'], " "), if show { "show" } else { "hide" }));
    fs::create_dir_all(config::lol_dir()).map_err(|e| e.to_string())?;
    fs::write(names_path(), rows.concat()).map_err(|e| e.to_string())
}

fn shows_repo(repo: &str) -> bool {
    repo_choice(repo).unwrap_or(false) // not asked (e.g. auto-share): hidden
}

// ------------------------------------------------------------ storage backends

fn rclone(args: &[&str]) -> Result<String, String> {
    let bin = config::which("rclone").ok_or("rclone is not installed")?;
    let out = config::cmd(bin.to_str().unwrap()).args(args.iter().take(1)).args(RCLONE_FLAGS).args(args.iter().skip(1)).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("rclone failed").to_string())
    }
}

fn put(t: &Target, src: &Path, rel: &str) -> Result<(), String> {
    match t {
        Target::Folder(base) => {
            let dst = base.join(rel);
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| format!("{}: {e}", base.display()))?;
            fs::copy(src, &dst).map(|_| ()).map_err(|e| format!("{}: {e}", dst.display()))
        }
        Target::Rclone(base) => rclone(&["copyto", src.to_str().unwrap(), &format!("{base}/{rel}")]).map(|_| ()),
    }
}

/// Picture and sidecar to <stem>.jpg / <stem>.tsv. On rclone one call for
/// both: one sign-in, the folder created once, both files in parallel.
fn put_pair(t: &Target, img: &Path, side: &Path, stem: &str) -> Result<(), String> {
    let Target::Rclone(base) = t else {
        return put(t, img, &format!("{stem}.jpg")).and_then(|_| put(t, side, &format!("{stem}.tsv")));
    };
    let (folder, name) = stem.rsplit_once('/').unwrap_or(("", stem));
    let dir = tmp_file(&format!("up-{:08x}", rand::thread_rng().gen::<u32>()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let res = fs::copy(img, dir.join(format!("{name}.jpg")))
        .and_then(|_| fs::copy(side, dir.join(format!("{name}.tsv"))))
        .map_err(|e| e.to_string())
        .and_then(|_| rclone(&["copy", dir.to_str().unwrap(), &format!("{base}/{folder}")]).map(|_| ()));
    let _ = fs::remove_dir_all(&dir);
    res
}

fn rm(t: &Target, rel: &str) -> Result<(), String> {
    match t {
        Target::Folder(base) => {
            let p = base.join(rel);
            match fs::remove_file(&p) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
                _ => {}
            }
            if let Some(dir) = p.parent() {
                let _ = fs::remove_dir(dir); // only if the repo folder is now empty
            }
            Ok(())
        }
        Target::Rclone(base) => rclone_rm(&format!("{base}/{rel}")),
    }
}

/// Delete a file on an rclone remote. Only managers of a Google shared drive may
/// delete for good (others get a 404), so fall back to Drive's trash, which
/// content managers may use. Ok once the file is gone.
fn rclone_rm(path: &str) -> Result<(), String> {
    let first = match rclone(&["deletefile", path]) {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };
    let _ = rclone(&["deletefile", "--drive-use-trash=true", path]);
    match rclone(&["lsf", path]) {
        Ok(out) if out.trim().is_empty() => Ok(()),
        Err(e) if e.contains("not found") => Ok(()),
        _ => Err(format!(
            "can't delete files there, so you couldn't take pictures back. \
             Ask a manager of the shared drive to make you at least \"Content manager\". ({first})"
        )),
    }
}

/// All files in the target as "<folder>/<file>".
fn list(t: &Target) -> Result<Vec<String>, String> {
    match t {
        Target::Folder(base) => {
            let mut out = Vec::new();
            for d in fs::read_dir(base).map_err(|e| format!("{}: {e}", base.display()))?.flatten() {
                if !d.path().is_dir() {
                    continue;
                }
                let folder = d.file_name().to_string_lossy().to_string();
                for f in fs::read_dir(d.path()).into_iter().flatten().flatten() {
                    out.push(format!("{folder}/{}", f.file_name().to_string_lossy()));
                }
            }
            Ok(out)
        }
        Target::Rclone(base) => Ok(rclone(&["lsf", "-R", "--files-only", "--max-depth", "2", base])?
            .lines()
            .filter(|l| l.contains('/'))
            .map(String::from)
            .collect()),
    }
}

/// "20261125-juergen-gross-1a2b3c4d.jpg" → ("20261125", "juergen-gross")
fn parse_name(file: &str) -> Option<(String, String)> {
    let stem = file.rsplit_once('.')?.0;
    let (date, rest) = stem.split_once('-')?;
    let (person, rand) = rest.rsplit_once('-')?;
    (date.len() == 8 && rand.len() == 8 && !person.is_empty()).then(|| (date.to_string(), person.to_string()))
}

/// My pictures in the target, as "<folder>/<stem>" (without extension).
fn mine(t: &Target, me: &str) -> Result<Vec<(String, String)>, String> {
    let mut v: Vec<(String, String)> = list(t)?
        .into_iter()
        .filter(|rel| rel.ends_with(".jpg"))
        .filter_map(|rel| {
            let file = rel.rsplit('/').next().unwrap_or("").to_string();
            let (date, person) = parse_name(&file)?;
            (person == me).then(|| (date, rel.trim_end_matches(".jpg").to_string()))
        })
        .collect();
    v.sort();
    Ok(v)
}

fn tmp_file(name: &str) -> PathBuf {
    let d = config::lol_dir().join(".preview");
    let _ = fs::create_dir_all(&d);
    d.join(name)
}

pub fn clear_previews() {
    let _ = fs::remove_dir_all(config::lol_dir().join(".preview"));
}

// ------------------------------------------------------------ share / unshare

/// The picture as it will be uploaded: no metadata, hash and repo label
/// blurred unless the person allowed this repo by name.
pub fn prepare(shot: &Shot) -> Result<PathBuf, String> {
    let img = render::load(Path::new(&shot.file))?;
    let img = if shows_repo(&shot.repo) {
        img
    } else {
        render::blur_rects(&img, &render::label_rects(img.width(), &shot.sha, &shot.repo, &shot.branch))
    };
    let out = tmp_file(&format!("{:08x}.jpg", rand::thread_rng().gen::<u32>()));
    render::save_jpeg(&img, &out)?;
    Ok(out)
}

/// Where a shot goes: its repo folder, or other-repos.
pub fn folder_for(shot: &Shot) -> String {
    if shows_repo(&shot.repo) { repo_folder(&shot.repo) } else { OTHER.into() }
}

pub fn upload(shot: &Shot) -> Result<(), String> {
    let t = target()?;
    let (name, me) = me()?;
    if store::shared_rows().iter().any(|(l, _)| l == &shot.file) {
        return Err("already shared".into());
    }
    let img = prepare(shot)?;
    let base = Path::new(&shot.file).file_name().unwrap().to_string_lossy().to_string();
    let d = base.get(..8).filter(|d| d.chars().all(|c| c.is_ascii_digit())).unwrap_or("00000000").to_string();
    let stem = format!("{}/{d}-{me}-{:08x}", folder_for(shot), rand::thread_rng().gen::<u32>());
    let repo = if shows_repo(&shot.repo) { shot.repo.replace('\t', " ") } else { "-".into() };
    let side = tmp_file("sidecar.tsv");
    fs::write(&side, format!("{}-{}-{}\t{}\t{}\t{}\n", &d[..4], &d[4..6], &d[6..8], name.replace('\t', " "), repo, shot.subject.replace(['\t', '\n'], " ")))
        .map_err(|e| e.to_string())?;
    let res = put_pair(&t, &img, &side, &stem);
    let _ = fs::remove_file(&img);
    let _ = fs::remove_file(&side);
    if let Err(e) = res {
        let _ = rm_pair(&t, &stem);
        return Err(e);
    }
    store::add_shared(&shot.file, &format!("{stem}.jpg")).map_err(|e| e.to_string())?;
    prune();
    Ok(())
}

/// lolcommits.share.retention days – off (0) unless the user sets it.
fn prune() {
    let days: i64 = config::get("share.retention").and_then(|d| d.parse().ok()).unwrap_or(0);
    if days <= 0 {
        return;
    }
    let (Ok(t), Ok((_, me))) = (target(), me()) else { return };
    let cutoff = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y%m%d").to_string();
    for (date, stem) in mine(&t, &me).unwrap_or_default() {
        if date < cutoff {
            let _ = rm_pair(&t, &stem);
            let _ = store::remove_shared(|_, r| r == format!("{stem}.jpg"));
        }
    }
}

/// Picture and sidecar at once: on Drive each delete is a round trip.
fn rm_pair(t: &Target, stem: &str) -> Result<(), String> {
    let (jpg, tsv) = std::thread::scope(|s| {
        let tsv = s.spawn(|| rm(t, &format!("{stem}.tsv")));
        (rm(t, &format!("{stem}.jpg")), tsv.join().unwrap_or_else(|_| Err("delete failed".into())))
    });
    jpg.and(tsv)
}

pub fn unshare(file: &str) -> Result<(), String> {
    let t = target()?;
    let rel = store::shared_rows().into_iter().find(|(l, _)| l == file).map(|r| r.1).ok_or("this picture is not shared")?;
    rm_pair(&t, rel.trim_end_matches(".jpg"))?;
    store::remove_shared(|l, _| l == file).map_err(|e| e.to_string())
}

/// Remove all of my pictures from the team gallery, from every computer.
pub fn unshare_all() -> Result<(), String> {
    let t = target()?;
    let (_, me) = me()?;
    for (_, stem) in mine(&t, &me)? {
        rm_pair(&t, &stem)?;
    }
    let _ = fs::remove_file(store::shared_path());
    Ok(())
}

#[derive(Serialize)]
pub struct TeamRow {
    pub date: String,
    pub name: String,
    pub repo: String,
    pub subject: String,
    pub file: String,
    pub mine: bool,
    /// When the picture was shared (file time), only to order pictures of the same day; never shown.
    #[serde(skip)]
    shared_at: std::time::SystemTime,
}

/// Folder to read team pictures from: the target itself for folders,
/// a downloaded copy for rclone remotes.
pub fn team_dir() -> Result<PathBuf, String> {
    Ok(match target()? {
        Target::Folder(p) => p,
        Target::Rclone(_) => config::lol_dir().join(".shared"),
    })
}

pub fn sync() -> Result<(), String> {
    if let Target::Rclone(base) = target()? {
        let dir = team_dir()?;
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        rclone(&["sync", &base, dir.to_str().unwrap()])?;
    }
    Ok(())
}

pub fn team() -> Result<Vec<TeamRow>, String> {
    let dir = team_dir()?;
    let me = config::get("share.name").map(|n| slug(&n)).unwrap_or_default();
    let days: i64 = config::get("share.retention").and_then(|d| d.parse().ok()).unwrap_or(0);
    let cutoff = if days > 0 { (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string() } else { String::new() };
    let mut rows = Vec::new();
    for rel in list(&Target::Folder(dir.clone()))? {
        let Some(stem) = rel.strip_suffix(".tsv") else { continue };
        let img = dir.join(format!("{stem}.jpg"));
        let Some(r) = store::tsv(&dir.join(&rel), 4).into_iter().next() else { continue };
        let file = stem.rsplit('/').next().unwrap_or("");
        if r[0] >= cutoff && img.is_file() {
            rows.push(TeamRow {
                date: r[0].clone(),
                name: r[1].clone(),
                repo: if r[2] == "-" { String::new() } else { r[2].clone() },
                subject: r[3].clone(),
                file: img.to_string_lossy().to_string(),
                mine: parse_name(&format!("{file}.jpg")).map(|(_, p)| p == me).unwrap_or(false),
                shared_at: fs::metadata(&img).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH),
            });
        }
    }
    // Newest day first; within a day the most recently shared first (the sidecar has no time of day).
    rows.sort_by(|a, b| (&b.date, b.shared_at, &b.file).cmp(&(&a.date, a.shared_at, &a.file)));
    Ok(rows)
}

/// Can we write to the target? Writes and deletes a small file.
pub fn probe(t: &Target) -> Result<(), String> {
    let tmp = tmp_file("probe.txt");
    fs::write(&tmp, "lolcommits connection test\n").map_err(|e| e.to_string())?;
    let rel = format!(".probe-{:08x}", rand::thread_rng().gen::<u32>());
    let res = put(t, &tmp, &rel).and_then(|_| match t {
        // probe file sits in the target root – remove it directly
        Target::Folder(base) => fs::remove_file(base.join(&rel)).map_err(|e| e.to_string()),
        Target::Rclone(base) => rclone_rm(&format!("{base}/{rel}")),
    });
    let _ = fs::remove_file(tmp);
    res.map_err(|e| format!("can't write there: {e}"))
}
