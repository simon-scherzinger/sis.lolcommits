//! Jobs from the git hook. `lolcommits capture` (the script inside the app)
//! writes ~/.lolcommits/queue/<YYYYmmdd-HHMMSS>-<sha>.tsv and returns at once;
//! the app takes the photo here, so `git commit` never waits for the camera.
use crate::store::{self, Shot};
use crate::{capture, config, render, share, tray};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};

/// Jobs older than this are dropped: you are probably not at the desk any more.
const STALE: Duration = Duration::from_secs(5 * 60);

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        let dir = config::lol_dir().join("queue");
        let mut jobs: Vec<_> = fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "tsv").unwrap_or(false)).collect())
            .unwrap_or_default();
        jobs.sort();
        for job in jobs {
            process(&app, &job);
        }
        std::thread::sleep(Duration::from_millis(400));
    });
}

fn process(app: &AppHandle, job: &Path) {
    let line = fs::read_to_string(job).unwrap_or_default();
    let age = fs::metadata(job).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
    let _ = fs::remove_file(job);
    let f: Vec<&str> = line.trim_end_matches('\n').split('\t').collect();
    if f.len() < 5 {
        return;
    }
    if age.map(|a| a > STALE).unwrap_or(false) {
        store::log(&format!("skipped stale job for {} {}", f[1], f[3]));
        return;
    }
    if config::paused_until().is_some() || !share::photos_on(f[1]) {
        return;
    }
    let stem = job.file_stem().unwrap().to_string_lossy().to_string(); // YYYYmmdd-HHMMSS-sha
    let (ts, repo, branch, sha, subject) = (f[0], f[1], f[2], f[3], f[4]);

    tray::start_animation(app);
    let res = (|| -> Result<Shot, String> {
        let img = capture::grab()?;
        let img = render::caption(render::fit(img), sha, subject, repo, branch);
        let out = config::lol_dir().join(repo).join(format!("{stem}.jpg"));
        render::save_jpeg(&img, &out)?;
        let shot = Shot {
            ts: ts.into(),
            repo: repo.into(),
            branch: branch.into(),
            sha: sha.into(),
            subject: subject.into(),
            file: out.to_string_lossy().to_string(),
            shared: false,
            private: false,
        };
        store::append(&shot).map_err(|e| e.to_string())?;
        store::prune_local();
        Ok(shot)
    })();
    tray::stop_animation();
    match res {
        Ok(shot) => {
            tray::set_error(app, None);
            if config::get_or("share.auto", "false") == "true" && share::configured() && share::may_share(repo) {
                if let Err(e) = share::upload(&shot, false) {
                    store::log(&format!("auto-share failed for {repo} {sha}: {e}"));
                }
            }
        }
        Err(e) => {
            store::log(&format!("capture failed for {repo} {sha}: {e}"));
            tray::set_error(app, Some(e));
        }
    }
    let _ = app.emit("shots-changed", ());
}
