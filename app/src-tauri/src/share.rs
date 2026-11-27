//! The team gallery, sorted by repo:
//!   <target>/<repo>/<YYYYmmdd>-<person>-<rand>.jpg
//!   <target>/.meta/<repo>/<YYYYmmdd>-<person>-<rand>.tsv   (date, name, repo or "-", subject)
//! The sidecars sit in the hidden .meta folder so the repo folders show only
//! pictures. Older versions put them next to the picture; those are still read
//! and deleted, and the app moves the person's own into .meta/ on start.
//! Repos the person didn't allow by name go to <target>/other-repos/ with the
//! repo label blurred. One sidecar per picture, so people sharing into the same
//! folder at the same time never edit the same file (no Drive conflict copies).
//! Targets: a folder (Google Drive for desktop, NAS) or an rclone remote.
use crate::store::{self, Shot};
use crate::{config, render, team as settings, time};
use rand::Rng;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const RCLONE_FLAGS: [&str; 1] = ["--drive-use-trash=false"];
pub const OTHER: &str = "other-repos";
/// Hidden folder for sidecars (and later team settings).
pub const META: &str = ".meta";

/// Sidecar of the picture <folder>/<name>: .meta/<folder>/<name>.tsv
fn side_rel(stem: &str) -> String {
    format!("{META}/{stem}.tsv")
}

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
// Show the repo name in the team gallery? Asked once per repo. With a team
// gallery it is a team setting (team.rs): the first person who shares from a
// repo decides for everyone. ~/.lolcommits/repo-names.tsv (repo \t show|hide)
// holds this computer's answers: used without a team gallery and until the
// team has a value.

fn names_path() -> PathBuf {
    config::lol_dir().join("repo-names.tsv")
}

fn local_choice(repo: &str) -> Option<bool> {
    store::tsv(&names_path(), 2).into_iter().find(|r| r[0] == repo).map(|r| r[1] == "show")
}

/// Some(true) = show by name, Some(false) = keep hidden, None = not asked yet
pub fn repo_choice(repo: &str) -> Option<bool> {
    team_choice(repo).or_else(|| local_choice(repo))
}

/// The team's choice, if there is a team gallery and it has one.
pub fn team_choice(repo: &str) -> Option<bool> {
    if configured() { settings::repo_choice(repo) } else { None }
}

pub fn local_repo_choices() -> Vec<(String, bool)> {
    store::tsv(&names_path(), 2).into_iter().map(|r| (r[0].clone(), r[1] == "show")).collect()
}

/// Change the choice – with a team gallery for everyone.
pub fn set_repo_choice(repo: &str, show: bool) -> Result<(), String> {
    if configured() {
        settings::set_repo_choice(repo, show)?;
    }
    set_local_choice(repo, show)
}

/// Answer to "show the repo name?" on the first share. If the team has
/// decided in the meantime, its choice stays and is returned.
pub fn answer_repo_choice(repo: &str, show: bool) -> Result<bool, String> {
    let show = if configured() {
        match settings::fresh_repo_choice(repo)? {
            Some(t) => t,
            None => {
                settings::set_repo_choice(repo, show)?;
                show
            }
        }
    } else {
        show
    };
    set_local_choice(repo, show)?;
    Ok(show)
}

fn set_local_choice(repo: &str, show: bool) -> Result<(), String> {
    let mut rows: Vec<String> = store::tsv(&names_path(), 2)
        .into_iter()
        .filter(|r| r[0] != repo)
        .map(|r| format!("{}\t{}\n", r[0], r[1]))
        .collect();
    rows.push(format!("{}\t{}\n", repo.replace(['\t', '\n'], " "), if show { "show" } else { "hide" }));
    fs::create_dir_all(config::lol_dir()).map_err(|e| e.to_string())?;
    fs::write(names_path(), rows.concat()).map_err(|e| e.to_string())
}

// Per-repo switches in the global git config, by repo name (so every clone
// and worktree of a repo counts), also read by the command line tool:
//   lolcommits.skipRepo   no photos in this repo
//   lolcommits.neverShare photos stay on this computer, not in Review
pub const SKIP: &str = "skipRepo";
pub const NEVER: &str = "neverShare";

pub fn photos_on(repo: &str) -> bool {
    !config::listed(SKIP, repo)
}

pub fn may_share(repo: &str) -> bool {
    !config::listed(NEVER, repo)
}

fn shows_repo(repo: &str) -> bool {
    repo_choice(repo).unwrap_or(false) // not asked (e.g. auto-share): hidden
}

// ------------------------------------------------------------ storage backends

pub(crate) fn rclone(args: &[&str]) -> Result<String, String> {
    let bin = config::which("rclone").ok_or("rclone is not installed")?;
    let out = config::cmd(bin.to_str().unwrap()).args(args.iter().take(1)).args(RCLONE_FLAGS).args(args.iter().skip(1)).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("rclone failed").to_string())
    }
}

pub(crate) fn put(t: &Target, src: &Path, rel: &str) -> Result<(), String> {
    match t {
        Target::Folder(base) => {
            let dst = base.join(rel);
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| format!("{}: {e}", base.display()))?;
            fs::copy(src, &dst).map(|_| ()).map_err(|e| format!("{}: {e}", dst.display()))
        }
        Target::Rclone(base) => rclone(&["copyto", src.to_str().unwrap(), &format!("{base}/{rel}")]).map(|_| ()),
    }
}

/// Picture and sidecar to <stem>.jpg / .meta/<stem>.tsv. On rclone one call
/// for both: one sign-in, the folders created once, both files in parallel.
fn put_pair(t: &Target, img: &Path, side: &Path, stem: &str) -> Result<(), String> {
    let Target::Rclone(base) = t else {
        return put(t, img, &format!("{stem}.jpg")).and_then(|_| put(t, side, &side_rel(stem)));
    };
    // the same layout in a temporary folder, copied over as a whole
    let dir = tmp_file(&format!("up-{:08x}", rand::thread_rng().gen::<u32>()));
    let res = [(img, dir.join(format!("{stem}.jpg"))), (side, dir.join(side_rel(stem)))]
        .iter()
        .try_for_each(|(src, dst)| fs::create_dir_all(dst.parent().unwrap()).and_then(|_| fs::copy(src, dst)).map(|_| ()))
        .map_err(|e| e.to_string())
        .and_then(|_| rclone(&["copy", dir.to_str().unwrap(), base]).map(|_| ()));
    let _ = fs::remove_dir_all(&dir);
    res
}

pub(crate) fn rm(t: &Target, rel: &str) -> Result<(), String> {
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

/// All files in the repo folders as "<folder>/<file>" (not in .meta/).
fn list(t: &Target) -> Result<Vec<String>, String> {
    match t {
        Target::Folder(base) => {
            let mut out = Vec::new();
            for d in fs::read_dir(base).map_err(|e| format!("{}: {e}", base.display()))?.flatten() {
                let folder = d.file_name().to_string_lossy().to_string();
                if !d.path().is_dir() || folder.starts_with('.') {
                    continue;
                }
                for f in fs::read_dir(d.path()).into_iter().flatten().flatten() {
                    out.push(format!("{folder}/{}", f.file_name().to_string_lossy()));
                }
            }
            Ok(out)
        }
        Target::Rclone(base) => Ok(rclone(&["lsf", "-R", "--files-only", "--max-depth", "2", base])?
            .lines()
            .filter(|l| l.contains('/') && !l.starts_with('.'))
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

pub(crate) fn tmp_file(name: &str) -> PathBuf {
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
    prepare_as(shot, shows_repo(&shot.repo))
}

fn prepare_as(shot: &Shot, show: bool) -> Result<PathBuf, String> {
    let img = render::load(Path::new(&shot.file))?;
    let img = if show {
        img
    } else {
        render::blur_rects(&img, &render::label_rects(img.width(), &shot.sha, &shot.repo, &shot.branch))
    };
    let out = tmp_file(&format!("{:08x}.jpg", rand::thread_rng().gen::<u32>()));
    render::save_jpeg(&img, &out)?;
    Ok(out)
}

/// Show the repo name for this upload? Asks the drive, not the mirror, so a
/// stale copy can't put a name in the drive that the team hides. Without a
/// team value yet this computer's answer becomes the team's.
fn show_for_upload(repo: &str) -> Result<bool, String> {
    if let Some(t) = settings::fresh_repo_choice(repo)? {
        return Ok(t);
    }
    match local_choice(repo) {
        Some(v) => settings::set_repo_choice(repo, v).map(|_| v),
        None => Ok(false), // not asked (e.g. auto-share): hidden, and nothing decided for the team
    }
}

/// `previewed`: the person saw the preview (made with repo_choice()); if the
/// team's choice changed meanwhile, nothing goes out and the preview must be
/// looked at again. False for auto-share, which has no preview.
pub fn upload(shot: &Shot, previewed: bool) -> Result<(), String> {
    let t = target()?;
    let (name, me) = me()?;
    if !may_share(&shot.repo) {
        return Err(format!("sharing is off for {} (Settings › Repos)", shot.repo));
    }
    if store::shared_rows().iter().any(|(l, _)| l == &shot.file) {
        return Err("already shared".into());
    }
    let seen = shows_repo(&shot.repo);
    let show = show_for_upload(&shot.repo)?;
    if previewed && show != seen {
        return Err(format!(
            "your team now {} “{}” by name – not shared, check the new preview",
            if show { "shows" } else { "hides" },
            shot.repo
        ));
    }
    let img = prepare_as(shot, show)?;
    let base = Path::new(&shot.file).file_name().unwrap().to_string_lossy().to_string();
    let d = base.get(..8).filter(|d| d.chars().all(|c| c.is_ascii_digit())).unwrap_or("00000000").to_string();
    let folder = if show { repo_folder(&shot.repo) } else { OTHER.into() };
    let stem = format!("{folder}/{d}-{me}-{:08x}", rand::thread_rng().gen::<u32>());
    let repo = if show { shot.repo.replace('\t', " ") } else { "-".into() };
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

/// Days to keep shared pictures (0 = keep): lolcommits.share.retention, at
/// most the team's maximum. A person can delete earlier, not keep longer.
pub fn retention_days() -> i64 {
    let mine: i64 = config::get("share.retention").and_then(|d| d.parse().ok()).filter(|d| *d > 0).unwrap_or(0);
    match (mine, if configured() { settings::retention() } else { None }) {
        (0, Some(t)) => t,
        (m, Some(t)) => m.min(t),
        (m, None) => m,
    }
}

fn prune() {
    let days = retention_days();
    if days <= 0 {
        return;
    }
    let (Ok(t), Ok((_, me))) = (target(), me()) else { return };
    let cutoff = time::days_ago(days).format("%Y%m%d").to_string();
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
        let tsv = s.spawn(|| rm(t, &side_rel(stem)));
        let old = s.spawn(|| rm_old_sidecar(t, stem));
        let jpg = rm(t, &format!("{stem}.jpg"));
        let _ = old.join();
        (jpg, tsv.join().unwrap_or_else(|_| Err("delete failed".into())))
    });
    jpg.and(tsv)
}

/// The sidecar older versions put next to the picture, if it is still there.
/// Best effort: start-up moves my own into .meta/. On rclone a filtered delete
/// succeeds without a match, so a missing file costs one call, not three.
fn rm_old_sidecar(t: &Target, stem: &str) {
    match t {
        Target::Folder(_) => {
            let _ = rm(t, &format!("{stem}.tsv"));
        }
        Target::Rclone(base) => {
            let (folder, name) = stem.rsplit_once('/').unwrap_or(("", stem));
            let (dir, only) = (format!("{base}/{folder}"), format!("/{name}.tsv"));
            if rclone(&["delete", &dir, "--include", &only]).is_err() {
                let _ = rclone(&["delete", "--drive-use-trash=true", &dir, "--include", &only]);
            }
        }
    }
}

fn mv(t: &Target, from: &str, to: &str) -> Result<(), String> {
    match t {
        Target::Folder(base) => {
            let dst = base.join(to);
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::rename(base.join(from), &dst).map_err(|e| format!("{}: {e}", dst.display()))
        }
        Target::Rclone(base) => rclone(&["moveto", &format!("{base}/{from}"), &format!("{base}/{to}")]).map(|_| ()),
    }
}

/// Move the sidecars of my pictures that older versions put next to the
/// picture into .meta/. Returns how many were moved.
pub fn move_old_sidecars() -> Result<usize, String> {
    let t = target()?;
    let (_, me) = me()?;
    let old: Vec<String> = list(&t)?
        .into_iter()
        .filter(|rel| rel.ends_with(".tsv"))
        .filter(|rel| parse_name(rel.rsplit('/').next().unwrap_or("")).is_some_and(|(_, p)| p == me))
        .collect();
    let mut moved = 0;
    let mut failed = None;
    for rel in &old {
        match mv(&t, rel, &format!("{META}/{rel}")) {
            Ok(()) => moved += 1,
            Err(e) => failed = Some(format!("{rel}: {e}")),
        }
    }
    match failed {
        Some(e) if moved == 0 => Err(e),
        Some(e) => {
            store::log(&format!("moving a sidecar into .meta/ failed: {e}"));
            Ok(moved)
        }
        None => Ok(moved),
    }
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
    let days = retention_days();
    let cutoff = if days > 0 { time::days_ago(days).format("%Y-%m-%d").to_string() } else { String::new() };
    let mut rows = Vec::new();
    for rel in list(&Target::Folder(dir.clone()))? {
        let Some(stem) = rel.strip_suffix(".jpg") else { continue };
        let img = dir.join(&rel);
        // sidecar in .meta/, or next to the picture (older versions)
        let side = [dir.join(side_rel(stem)), dir.join(format!("{stem}.tsv"))].into_iter().find(|p| p.is_file());
        let Some(r) = side.and_then(|p| store::tsv(&p, 4).into_iter().next()) else { continue };
        let file = stem.rsplit('/').next().unwrap_or("");
        if r[0] >= cutoff {
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
