// lolcommits.app – menu bar app that takes the commit selfies.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod config;
mod drive;
mod queue;
mod render;
mod share;
mod store;
mod tray;
mod update;
#[cfg(test)]
mod tests;

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

type R<T> = Result<T, String>;

/// Settings the UI may change directly. Share targets only change through the
/// setup commands below, which test the connection first.
const KEYS: [&str; 9] = [
    "device", "delay", "retention", "share.name", "share.retention", "share.auto", "update.auto",
    "share.googleClientId", "share.googleClientSecret",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct State {
    version: String,
    dir: String,
    config: HashMap<String, String>,
    hook_global: bool,
    hook_ours: bool,
    config_hooks: bool,
    git_version: String,
    platform: String,
    shots: Vec<store::Shot>,
    sharing: bool,
    share_label: String,
    share_error: String,
    setup_done: bool,
    paused_until: Option<i64>,
    camera_ok: bool,
    autostart: bool,
    ffmpeg: bool,
}

fn script_path(app: &AppHandle) -> PathBuf {
    app.path().resource_dir().map(|d| d.join("lolcommits")).unwrap_or_default()
}

fn hook_value(app: &AppHandle) -> String {
    format!("/bin/bash \"{}\" capture", script_path(app).display())
}

fn allow_assets(app: &AppHandle) {
    let scope = app.asset_protocol_scope();
    let dir = config::lol_dir();
    let _ = scope.allow_directory(&dir, true);
    // "**" doesn't match dot folders on unix, so name them explicitly
    let _ = scope.allow_directory(dir.join(".preview"), true);
    let _ = scope.allow_directory(dir.join(".shared"), true);
    if let Ok(d) = share::team_dir() {
        let _ = scope.allow_directory(d, true);
    }
}

#[tauri::command(async)]
fn get_state(app: AppHandle) -> State {
    let cfg = config::all();
    let hook = config::hook_command();
    State {
        version: app.package_info().version.to_string(),
        dir: config::lol_dir().to_string_lossy().to_string(),
        hook_global: hook.is_some(),
        hook_ours: hook.as_deref() == Some(hook_value(&app).as_str()),
        config_hooks: config::has_config_hooks(),
        git_version: config::git_version(),
        platform: std::env::consts::OS.into(),
        shots: store::shots(),
        sharing: share::configured(),
        share_label: cfg.get("share.label").cloned().or_else(|| cfg.get("share.target").cloned()).unwrap_or_default(),
        share_error: match share::target() {
            Err(e) if cfg.contains_key("share.target") => e,
            _ => String::new(),
        },
        setup_done: cfg.contains_key("setup"),
        paused_until: config::paused_until(),
        camera_ok: capture::permission_granted(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        ffmpeg: config::which("ffmpeg").is_some(),
        config: cfg,
    }
}

#[tauri::command(async)]
fn set_config(app: AppHandle, key: String, value: String) -> R<()> {
    if !KEYS.contains(&key.as_str()) || value.contains('\n') {
        return Err("unknown setting".into());
    }
    config::set(&key, value.trim())?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
fn enable_global(app: AppHandle) -> R<()> {
    if !config::has_config_hooks() {
        return Err(format!("git {} has no global hooks – git 2.54 or newer is needed", config::git_version()));
    }
    let _ = config::git_global(&["--unset-all", "hook.lolcommits.event"]);
    config::git_global(&["hook.lolcommits.event", "post-commit"])?;
    config::git_global(&["hook.lolcommits.command", &hook_value(&app)])?;
    Ok(())
}

#[tauri::command(async)]
fn disable_global() -> R<()> {
    let _ = config::git_global(&["--remove-section", "hook.lolcommits"]);
    Ok(())
}

/// minutes > 0: pause that long; 0: until resumed; -1: until tomorrow 6:00
#[tauri::command(async)]
fn pause(app: AppHandle, minutes: i64) -> R<()> {
    pause_for(minutes)?;
    tray::refresh(&app);
    let _ = app.emit("shots-changed", ());
    Ok(())
}

fn pause_for(minutes: i64) -> R<()> {
    match minutes {
        0 => config::set("enabled", "false"),
        -1 => {
            let tomorrow = (chrono::Local::now() + chrono::Duration::days(1)).date_naive().and_hms_opt(6, 0, 0).unwrap();
            let t = tomorrow.and_local_timezone(chrono::Local).single().map(|d| d.timestamp()).unwrap_or(config::now_unix() + 86400);
            config::set("pausedUntil", &t.to_string())
        }
        m => config::set("pausedUntil", &(config::now_unix() + m * 60).to_string()),
    }
}

#[tauri::command(async)]
fn resume(app: AppHandle) -> R<()> {
    config::set("enabled", "")?;
    config::set("pausedUntil", "")?;
    tray::refresh(&app);
    let _ = app.emit("shots-changed", ());
    Ok(())
}

#[tauri::command(async)]
fn camera_permission() -> bool {
    capture::request_permission()
}

#[tauri::command(async)]
fn devices() -> Vec<capture::Device> {
    capture::devices()
}

#[tauri::command(async)]
fn test_shot(app: AppHandle) -> R<String> {
    tray::start_animation(&app);
    let res = capture::grab().and_then(|img| {
        let img = render::caption(render::fit(img), "c0ffee1337", "lolcommits test shot – looking good", "lolcommits", "main");
        let out = config::lol_dir().join("test.jpg");
        render::save_jpeg(&img, &out).map(|_| out.to_string_lossy().to_string())
    });
    tray::stop_animation();
    tray::set_error(&app, res.as_ref().err().cloned());
    allow_assets(&app);
    res
}

fn known(file: &str) -> R<store::Shot> {
    store::find(file).ok_or_else(|| "unknown picture".into())
}

/// The picture exactly as it would be uploaded (blurred band, no metadata).
#[tauri::command(async)]
fn preview(app: AppHandle, file: String) -> R<String> {
    let shot = known(&file)?;
    allow_assets(&app);
    share::prepare(&shot).map(|p| p.to_string_lossy().to_string())
}

#[derive(Serialize)]
struct RepoChoice {
    repo: String,
    /// true: shown by name, false: hidden (other-repos), null: not asked yet
    show: Option<bool>,
    folder: String,
}

/// Every repo with photos, and whether it may appear by name in the team gallery.
#[tauri::command(async)]
fn repo_choices() -> Vec<RepoChoice> {
    let mut repos: Vec<String> = store::shots().into_iter().map(|s| s.repo).collect();
    repos.extend(share::repo_choices().into_iter().map(|r| r.0));
    repos.sort_by_key(|r| r.to_lowercase());
    repos.dedup();
    repos
        .into_iter()
        .map(|r| {
            let show = share::repo_choice(&r);
            let folder = if show == Some(true) { share::repo_folder(&r) } else { share::OTHER.to_string() };
            RepoChoice { repo: r, show, folder }
        })
        .collect()
}

/// Asked once per repo before its first share; changeable in Settings.
/// Applies to pictures shared from now on.
#[tauri::command(async)]
fn set_repo_choice(repo: String, show: bool) -> R<()> {
    share::set_repo_choice(&repo, show)
}

/// Review: swipe left keeps a picture on this computer and out of the review.
#[tauri::command(async)]
fn set_private(app: AppHandle, file: String, private: bool) -> R<()> {
    known(&file)?;
    store::set_private(&file, private).map_err(|e| e.to_string())?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
fn share_shot(app: AppHandle, file: String) -> R<()> {
    let r = share::upload(&known(&file)?);
    if r.is_ok() {
        let _ = store::set_private(&file, false);
    }
    tray::refresh(&app);
    r
}

#[tauri::command(async)]
fn unshare(app: AppHandle, file: Option<String>) -> R<()> {
    let r = match file {
        Some(f) => share::unshare(&f),
        None => share::unshare_all(),
    };
    tray::refresh(&app);
    r
}

#[tauri::command(async)]
fn forget(app: AppHandle, file: Option<String>, also_shared: bool) -> R<()> {
    match file {
        Some(f) => {
            let shot = known(&f)?;
            if shot.shared && also_shared {
                share::unshare(&f)?;
            } else if shot.shared {
                store::remove_shared(|l, _| l == f).map_err(|e| e.to_string())?;
            }
            store::remove(&f).map_err(|e| e.to_string())?;
        }
        None => {
            if also_shared && share::configured() {
                share::unshare_all()?;
            }
            store::forget_all();
        }
    }
    tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
fn team(app: AppHandle, refresh: bool) -> R<Vec<share::TeamRow>> {
    if refresh {
        share::sync()?;
    }
    allow_assets(&app);
    share::team()
}

#[tauri::command(async)]
fn wrapped(year: String) -> R<String> {
    let files: Vec<String> = store::shots().into_iter().rev().filter(|s| s.ts.starts_with(&year)).map(|s| s.file).collect();
    let sheet = render::contact_sheet(&files).map_err(|_| format!("no lolcommits in {year}"))?;
    let out = config::lol_dir().join(format!("wrapped-{year}.jpg"));
    render::save_jpeg(&sheet, &out)?;
    Ok(out.to_string_lossy().to_string())
}

#[tauri::command(async)]
fn timelapse(repo: Option<String>) -> R<String> {
    let ff = config::which("ffmpeg").ok_or("The timelapse video needs ffmpeg. Install it with Homebrew: brew install ffmpeg")?;
    let repo = repo.filter(|r| !r.is_empty());
    let mut shots: Vec<_> = store::shots().into_iter().filter(|s| repo.as_ref().map(|r| &s.repo == r).unwrap_or(true)).collect();
    shots.reverse();
    if shots.is_empty() {
        return Err("no pictures".into());
    }
    let list = config::lol_dir().join(".timelapse.txt");
    let body: String = shots.iter().map(|s| format!("file '{}'\nduration 0.2\n", s.file.replace('\'', "'\\''"))).collect();
    std::fs::write(&list, body).map_err(|e| e.to_string())?;
    let out = config::lol_dir().join(match &repo { Some(r) => format!("timelapse-{r}.mp4"), None => "timelapse.mp4".into() });
    let o = config::cmd(ff.to_str().unwrap())
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-vf", "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,fps=25,format=yuv420p", "-c:v", "libx264", "-movflags", "+faststart"])
        .arg(&out)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&list);
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("ffmpeg failed").to_string());
    }
    Ok(out.to_string_lossy().to_string())
}

// ---------------------------------------------------------------- team setup

#[tauri::command(async)]
fn drive_folders() -> Vec<drive::DriveFolder> {
    drive::folders()
}

fn set_target(app: &AppHandle, target: &str, label: &str) -> R<()> {
    config::set("share.target", target)?;
    config::set("share.label", label)?;
    allow_assets(app);
    tray::refresh(app);
    Ok(())
}

/// Use a folder (a Drive for desktop shared drive, a NAS, ...): pictures go to <folder>/lolcommits.
#[tauri::command(async)]
fn use_folder(app: AppHandle, path: String, label: String) -> R<()> {
    let base = Path::new(&path);
    if !base.is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    let dir = if base.file_name().map(|n| n == "lolcommits").unwrap_or(false) { base.to_path_buf() } else { base.join("lolcommits") };
    std::fs::create_dir_all(&dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    share::probe(&share::Target::Folder(dir.clone()))?;
    set_target(&app, &dir.to_string_lossy(), &label)
}

#[derive(Serialize)]
struct RcloneStatus {
    installed: bool,
    connected: bool,
    brew: bool,
    /// Google client for the sign-in: "own", "builtin" or "none"
    client: &'static str,
}

#[tauri::command(async)]
fn rclone_status() -> RcloneStatus {
    let installed = drive::rclone_installed();
    RcloneStatus { installed, connected: installed && drive::signed_in(), brew: config::which("brew").is_some(), client: drive::client_source() }
}

#[tauri::command(async)]
fn brew_install(pkg: String) -> R<()> {
    if !["rclone", "ffmpeg"].contains(&pkg.as_str()) {
        return Err("not allowed".into());
    }
    let brew = config::which("brew").ok_or("Homebrew is not installed – see https://brew.sh")?;
    let out = config::cmd(brew.to_str().unwrap()).args(["install", &pkg]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("brew failed").to_string())
    }
}

#[tauri::command(async)]
fn rclone_connect() -> R<Vec<drive::SharedDrive>> {
    drive::connect()
}

#[tauri::command(async)]
fn rclone_cancel() {
    drive::cancel_login()
}

#[tauri::command(async)]
fn rclone_use(app: AppHandle, id: String, name: String) -> R<()> {
    let target = drive::target_for(&id);
    let t = share::Target::Rclone(target.trim_start_matches("rclone:").to_string());
    share::probe(&t)?;
    set_target(&app, &target, &format!("Google Drive › {name} › lolcommits"))
}

/// Stop sharing from this computer. Pictures already in the team folder stay.
#[tauri::command(async)]
fn disconnect_sharing(app: AppHandle) -> R<()> {
    if config::get("share.target").map(|t| t.starts_with(&format!("rclone:{}", drive::REMOTE))).unwrap_or(false) {
        drive::disconnect();
    }
    config::set("share.target", "")?;
    config::set("share.label", "")?;
    config::set("share.auto", "")?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
fn finish_setup(app: AppHandle) -> R<()> {
    config::set("setup", &app.package_info().version.to_string())
}

#[tauri::command(async)]
fn set_autostart(app: AppHandle, on: bool) -> R<()> {
    let r = if on { app.autolaunch().enable() } else { app.autolaunch().disable() };
    tray::refresh(&app);
    r.map_err(|e| e.to_string())
}

/// The team privacy guide (PRIVACY.md), bundled so it works offline.
#[tauri::command(async)]
fn privacy_doc(app: AppHandle) -> R<String> {
    let p = app.path().resource_dir().map_err(|e| e.to_string())?.join("PRIVACY.md");
    std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
}

#[tauri::command(async)]
fn first_name() -> String {
    config::user_first_name()
}

/// Optional: make the `lolcommits` command line tool available in ~/.local/bin.
#[tauri::command(async)]
fn install_cli(app: AppHandle) -> R<String> {
    let bin = config::home().join(".local/bin");
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let link = bin.join("lolcommits");
    if link.exists() && std::fs::symlink_metadata(&link).map(|m| !m.file_type().is_symlink()).unwrap_or(false) {
        return Err(format!("{} exists and is not a link – remove it first", link.display()));
    }
    let _ = std::fs::remove_file(&link);
    #[cfg(unix)]
    std::os::unix::fs::symlink(script_path(&app), &link).map_err(|e| e.to_string())?;
    Ok(link.to_string_lossy().to_string())
}

// ---------------------------------------------------------------- updates

#[tauri::command(async)]
fn update_status(app: AppHandle) -> update::Status {
    update::status(&app)
}

#[tauri::command]
async fn update_check(app: AppHandle) -> R<Option<String>> {
    update::check(&app).await
}

#[tauri::command]
async fn update_install(app: AppHandle) -> R<()> {
    update::install(&app).await
}

// ---------------------------------------------------------------- window, tray

fn show_main(app: &AppHandle, route: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        if let Some(r) = route {
            let _ = w.emit("navigate", r);
        }
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app, Some("shots")),
        "settings" => show_main(app, Some("settings")),
        "error" => show_main(app, Some("settings")),
        "review" => show_main(app, Some("review")),
        "test" => {
            let a = app.clone();
            std::thread::spawn(move || {
                if let Ok(f) = test_shot(a.clone()) {
                    show_main(&a, None);
                    let _ = a.emit("show-file", f);
                }
            });
        }
        "pause-60" => { let _ = pause(app.clone(), 60); }
        "pause-tomorrow" => { let _ = pause(app.clone(), -1); }
        "pause-forever" => { let _ = pause(app.clone(), 0); }
        "resume" => { let _ = resume(app.clone()); }
        "autostart" => {
            let on = app.autolaunch().is_enabled().unwrap_or(false);
            let _ = set_autostart(app.clone(), !on);
        }
        "update" => {
            let a = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = update::install(&a).await {
                    store::log(&format!("update failed: {e}"));
                    show_main(&a, Some("settings"));
                }
            });
        }
        "check-update" => {
            show_main(app, Some("settings"));
            let a = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = update::check(&a).await;
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Tell the UI when pictures change (also from the CLI) and keep the menu's
/// "last photo … ago" and pause state current.
fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let stamp = || {
            ["index.tsv", "shared.tsv"]
                .iter()
                .map(|f| std::fs::metadata(config::lol_dir().join(f)).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH))
                .collect::<Vec<_>>()
        };
        let mut last = stamp();
        let mut ticks = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(1500));
            ticks += 1;
            let now = stamp();
            if now != last {
                last = now;
                let _ = app.emit("shots-changed", ());
                tray::refresh(&app);
            } else if ticks % 40 == 0 {
                tray::refresh(&app); // once a minute
            }
        }
    });
}

fn main() {
    let background = std::env::args().any(|a| a == "--background");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app, None)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .invoke_handler(tauri::generate_handler![
            get_state, set_config, enable_global, disable_global, pause, resume, camera_permission, devices,
            test_shot, preview, share_shot, unshare, forget, team, wrapped, timelapse, drive_folders, use_folder,
            rclone_status, brew_install, rclone_connect, rclone_use, disconnect_sharing, finish_setup,
            set_autostart, first_name, install_cli, privacy_doc, set_private,
            repo_choices, set_repo_choice, update_status, update_check, update_install
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            share::clear_previews();
            allow_assets(&handle);
            let menu = tray::build_menu(&handle)?;
            TrayIconBuilder::with_id("main")
                .icon(tray::idle_icon())
                .icon_as_template(true)
                .tooltip("lolcommits")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, e| on_menu(app, e.id.as_ref()))
                .build(app)?;
            queue::start(handle.clone());
            watch(handle.clone());
            update::start(handle.clone());
            let setup_done = config::get("setup").is_some();
            if !background || !setup_done {
                show_main(&handle, None);
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            // closing the window keeps lolcommits running in the menu bar
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building lolcommits")
        .run(|app, e| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = e {
                show_main(app, None);
            }
            let _ = (app, e);
        });
}
