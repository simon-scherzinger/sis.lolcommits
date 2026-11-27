//! The local picture store: ~/.lolcommits/<repo>/<file>.jpg plus index.tsv and
//! shared.tsv – the same files the command line tool reads and writes.
use crate::{config, time};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
pub struct Shot {
    pub ts: String,
    pub repo: String,
    pub branch: String,
    pub sha: String,
    pub subject: String,
    pub file: String,
    pub shared: bool,
    /// swiped left in the review: stays on this computer, not offered again
    pub private: bool,
}

pub fn index_path() -> PathBuf {
    config::lol_dir().join("index.tsv")
}
pub fn shared_path() -> PathBuf {
    config::lol_dir().join("shared.tsv")
}
pub fn private_path() -> PathBuf {
    config::lol_dir().join("private.tsv")
}
pub fn log(msg: &str) {
    let dir = config::lol_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(dir.join("lolcommits.log")) {
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z"), msg);
    }
}

pub fn tsv(path: &Path, n: usize) -> Vec<Vec<String>> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.split('\t').map(String::from).collect::<Vec<_>>())
        .filter(|r| r.len() >= n)
        .collect()
}

fn write_lines(path: &Path, rows: &[Vec<String>]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut s = String::new();
    for r in rows {
        s.push_str(&r.join("\t"));
        s.push('\n');
    }
    fs::write(&tmp, s)?;
    fs::rename(tmp, path)
}

/// (local file, remote path) of pictures shared from this computer
pub fn shared_rows() -> Vec<(String, String)> {
    tsv(&shared_path(), 2).into_iter().map(|r| (r[0].clone(), r[1].clone())).collect()
}

pub fn private_files() -> Vec<String> {
    tsv(&private_path(), 1).into_iter().map(|r| r[0].clone()).collect()
}

pub fn set_private(file: &str, private: bool) -> std::io::Result<()> {
    let mut files: Vec<String> = private_files().into_iter().filter(|f| f != file).collect();
    if private {
        files.push(file.to_string());
    }
    fs::create_dir_all(config::lol_dir())?;
    let rows: Vec<Vec<String>> = files.into_iter().map(|f| vec![f]).collect();
    write_lines(&private_path(), &rows)
}

/// Photos waiting in Review: neither shared nor kept private, from repos that may be shared.
pub fn to_review(shots: &[Shot]) -> usize {
    let never = config::get_all(crate::share::NEVER);
    shots.iter().filter(|s| !s.shared && !s.private && !never.contains(&s.repo)).count()
}

pub fn shots() -> Vec<Shot> {
    let shared: Vec<String> = shared_rows().into_iter().map(|r| r.0).collect();
    let private = private_files();
    let mut v: Vec<Shot> = tsv(&index_path(), 6)
        .into_iter()
        .filter(|r| Path::new(&r[5]).is_file())
        .map(|r| Shot {
            shared: shared.contains(&r[5]),
            private: private.contains(&r[5]),
            ts: r[0].clone(),
            repo: r[1].clone(),
            branch: r[2].clone(),
            sha: r[3].clone(),
            subject: r[4].clone(),
            file: r[5].clone(),
        })
        .collect();
    v.sort_by(|a, b| b.ts.cmp(&a.ts));
    v
}

pub fn find(file: &str) -> Option<Shot> {
    shots().into_iter().find(|s| s.file == file)
}

pub fn append(shot: &Shot) -> std::io::Result<()> {
    let p = index_path();
    fs::create_dir_all(p.parent().unwrap())?;
    let mut f = fs::OpenOptions::new().create(true).append(true).open(p)?;
    let clean = |s: &str| s.replace(['\t', '\n'], " ");
    writeln!(
        f,
        "{}\t{}\t{}\t{}\t{}\t{}",
        shot.ts,
        clean(&shot.repo),
        clean(&shot.branch),
        shot.sha,
        clean(&shot.subject),
        shot.file
    )
}

pub fn remove(file: &str) -> std::io::Result<()> {
    let _ = fs::remove_file(file);
    let rows: Vec<_> = tsv(&index_path(), 6).into_iter().filter(|r| r[5] != file).collect();
    if private_path().exists() {
        let _ = set_private(file, false);
    }
    write_lines(&index_path(), &rows)
}

pub fn add_shared(local: &str, remote: &str) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(shared_path())?;
    writeln!(f, "{local}\t{remote}")
}

pub fn remove_shared(pred: impl Fn(&str, &str) -> bool) -> std::io::Result<()> {
    let rows: Vec<Vec<String>> = tsv(&shared_path(), 2).into_iter().filter(|r| !pred(&r[0], &r[1])).collect();
    if shared_path().exists() {
        write_lines(&shared_path(), &rows)?;
    }
    Ok(())
}

/// lolcommits.retention: delete local pictures older than N days (unset = keep)
pub fn prune_local() {
    let days: i64 = config::get("retention").and_then(|d| d.parse().ok()).unwrap_or(0);
    if days <= 0 {
        return;
    }
    let cutoff = time::days_ago(days).format("%Y-%m-%d").to_string();
    let (old, keep): (Vec<_>, Vec<_>) =
        tsv(&index_path(), 6).into_iter().partition(|r| r[0].get(..10).unwrap_or("") < cutoff.as_str());
    if old.is_empty() {
        return;
    }
    for r in &old {
        let _ = fs::remove_file(&r[5]);
    }
    let _ = write_lines(&index_path(), &keep);
}

/// Delete every local picture and the files that describe them.
pub fn forget_all() {
    for r in tsv(&index_path(), 6) {
        let _ = fs::remove_file(&r[5]);
    }
    let dir = config::lol_dir();
    for f in ["index.tsv", "shared.tsv", "private.tsv", "lolcommits.log", "test.jpg", "gallery.html", "gallery-shared.html", "timelapse.mp4"] {
        let _ = fs::remove_file(dir.join(f));
    }
    for d in [".shared", ".preview"] {
        let _ = fs::remove_dir_all(dir.join(d));
    }
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_file() && (name.starts_with("wrapped-") || name.starts_with("timelapse")) {
                let _ = fs::remove_file(&p);
            }
            if p.is_dir() {
                let _ = fs::remove_dir(&p); // only removes empty repo folders
            }
        }
    }
}
