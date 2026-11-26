//! Menu bar icon: drawn at runtime as a template image (macOS tints it for
//! light/dark menu bars). The lens closes like a shutter and a flash fires
//! while a photo is being taken.
use crate::{config, share, store, update};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_line_segment_mut};
use imageproc::rect::Rect;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Wry};

const S: u32 = 44; // 22pt @2x
const INK: Rgba<u8> = Rgba([0, 0, 0, 255]);
const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);

pub static ANIMATING: AtomicBool = AtomicBool::new(false);
pub static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn rounded_rect(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, r: i32, c: Rgba<u8>) {
    draw_filled_rect_mut(img, Rect::at(x0 + r, y0).of_size((x1 - x0 - 2 * r) as u32, (y1 - y0) as u32), c);
    draw_filled_rect_mut(img, Rect::at(x0, y0 + r).of_size((x1 - x0) as u32, (y1 - y0 - 2 * r) as u32), c);
    for (cx, cy) in [(x0 + r, y0 + r), (x1 - r - 1, y0 + r), (x0 + r, y1 - r - 1), (x1 - r - 1, y1 - r - 1)] {
        draw_filled_circle_mut(img, (cx, cy), r, c);
    }
}

fn thick_line(img: &mut RgbaImage, a: (f32, f32), b: (f32, f32), w: i32, c: Rgba<u8>) {
    for dx in -w..=w {
        for dy in -w..=w {
            if dx * dx + dy * dy <= w * w {
                draw_line_segment_mut(img, (a.0 + dx as f32, a.1 + dy as f32), (b.0 + dx as f32, b.1 + dy as f32), c);
            }
        }
    }
}

/// lens: 0..=6 (6 = wide open), flash: 0..=2, slash: paused, dot: error
fn draw(lens: i32, flash: i32, slash: bool, dot: bool) -> Image<'static> {
    let mut img = RgbaImage::from_pixel(S, S, CLEAR);
    rounded_rect(&mut img, 4, 13, 40, 37, 5, INK); // body
    rounded_rect(&mut img, 15, 9, 27, 15, 2, INK); // viewfinder bump
    draw_filled_circle_mut(&mut img, (22, 25), 9, CLEAR); // lens ring
    if lens > 0 {
        draw_filled_circle_mut(&mut img, (22, 25), lens, INK);
    }
    if flash > 0 {
        // flash rays top right
        let l = 3.0 + 2.5 * flash as f32;
        for (dx, dy) in [(1.0, -1.0), (1.0, 0.0), (0.0, -1.0)] {
            let (x, y) = (37.0, 8.0);
            thick_line(&mut img, (x + dx * 2.0, y + dy * 2.0), (x + dx * l, y + dy * l), 1, INK);
        }
    }
    if slash {
        thick_line(&mut img, (6.0, 41.0), (40.0, 5.0), 4, CLEAR);
        thick_line(&mut img, (6.0, 41.0), (40.0, 5.0), 1, INK);
        // paused: dimmed, like a disabled menu bar item
        for p in img.pixels_mut() {
            p.0[3] = (p.0[3] as u16 * 2 / 5) as u8;
        }
    }
    if dot {
        draw_filled_circle_mut(&mut img, (38, 9), 6, CLEAR);
        draw_filled_circle_mut(&mut img, (38, 9), 4, INK);
    }
    Image::new_owned(img.into_raw(), S, S)
}

pub fn idle_icon() -> Image<'static> {
    let paused = config::paused_until().is_some();
    let err = LAST_ERROR.lock().map(|e| e.is_some()).unwrap_or(false);
    draw(5, 0, paused, err && !paused)
}

fn set_icon(tray: &TrayIcon<Wry>, img: Image<'static>) {
    let _ = tray.set_icon(Some(img));
    let _ = tray.set_icon_as_template(true);
}

/// Shutter animation until stop_animation().
pub fn start_animation(app: &AppHandle) {
    if ANIMATING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let frames = [(5, 0), (3, 1), (1, 2), (0, 2), (1, 2), (3, 1)];
        let mut i = 0;
        while ANIMATING.load(Ordering::SeqCst) {
            if let Some(tray) = app.tray_by_id("main") {
                let (lens, flash) = frames[i % frames.len()];
                set_icon(&tray, draw(lens, flash, false, false));
            }
            i += 1;
            std::thread::sleep(Duration::from_millis(110));
        }
        refresh(&app);
    });
}

pub fn stop_animation() {
    ANIMATING.store(false, Ordering::SeqCst);
}

pub fn set_error(app: &AppHandle, err: Option<String>) {
    if let Ok(mut e) = LAST_ERROR.lock() {
        *e = err;
    }
    refresh(app);
}

fn ago(ts: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(ts) else { return String::new() };
    let mins = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_minutes();
    match mins {
        m if m < 1 => "just now".into(),
        m if m < 60 => format!("{m} min ago"),
        m if m < 60 * 24 => format!("{} h ago", m / 60),
        m => format!("{} days ago", m / 60 / 24),
    }
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let shots = store::shots();
    let paused = config::paused_until();
    let status = match (&paused, shots.first()) {
        (Some(t), _) if *t == i64::MAX => "⏸ Paused".to_string(),
        (Some(t), _) => {
            let until = chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local)).map(|d| d.format("%H:%M").to_string()).unwrap_or_default();
            format!("⏸ Paused until {until}")
        }
        (None, Some(s)) => format!("Last photo {} – {}", ago(&s.ts), short(&s.subject, 28)),
        (None, None) => "No photos yet – commit something".into(),
    };
    let err = LAST_ERROR.lock().ok().and_then(|e| e.clone());

    let status_item = MenuItem::with_id(app, "status", status, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open lolcommits", true, None::<&str>)?;
    let test = MenuItem::with_id(app, "test", "Take a test photo", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    use tauri_plugin_autostart::ManagerExt;
    let login_on = app.autolaunch().is_enabled().unwrap_or(false);
    let login = CheckMenuItem::with_id(app, "autostart", "Start at login", true, login_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit lolcommits", true, None::<&str>)?;

    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<Wry>>> = vec![Box::new(status_item)];
    if let Some(e) = err {
        items.push(Box::new(MenuItem::with_id(app, "error", format!("⚠ {}", short(&e, 48)), true, None::<&str>)?));
    }
    items.push(Box::new(sep1));
    items.push(Box::new(open));
    items.push(Box::new(test));
    if paused.is_some() {
        items.push(Box::new(MenuItem::with_id(app, "resume", "Resume", true, None::<&str>)?));
    } else {
        let h1 = MenuItem::with_id(app, "pause-60", "For 1 hour", true, None::<&str>)?;
        let tm = MenuItem::with_id(app, "pause-tomorrow", "Until tomorrow", true, None::<&str>)?;
        let fv = MenuItem::with_id(app, "pause-forever", "Until I resume", true, None::<&str>)?;
        items.push(Box::new(Submenu::with_id_and_items(app, "pause", "Pause", true, &[&h1, &tm, &fv])?));
    }
    let to_review = shots.iter().filter(|s| !s.shared && !s.private).count();
    if share::configured() && to_review > 0 {
        let label = if to_review == 1 { "Review 1 new photo…".to_string() } else { format!("Review {to_review} new photos…") };
        items.push(Box::new(MenuItem::with_id(app, "review", label, true, None::<&str>)?));
    }
    items.push(Box::new(sep2));
    let up = update::status(app);
    let up_item = match (&up.progress, &up.available) {
        (Some(p), _) => MenuItem::with_id(app, "updating", format!("Updating… {p}%"), false, None::<&str>)?,
        (None, Some(v)) => MenuItem::with_id(app, "update", format!("Update to {v} and restart"), true, None::<&str>)?,
        _ => MenuItem::with_id(app, "check-update", "Check for updates…", true, None::<&str>)?,
    };
    items.push(Box::new(up_item));
    items.push(Box::new(settings));
    items.push(Box::new(login));
    items.push(Box::new(sep3));
    items.push(Box::new(quit));
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Update icon, tooltip and menu (status line, pause state).
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else { return };
    if !ANIMATING.load(Ordering::SeqCst) {
        set_icon(&tray, idle_icon());
    }
    let tip = match LAST_ERROR.lock().ok().and_then(|e| e.clone()) {
        Some(e) => format!("lolcommits – {e}"),
        None if config::paused_until().is_some() => "lolcommits – paused".into(),
        None => "lolcommits".into(),
    };
    let _ = tray.set_tooltip(Some(tip));
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = app.get_webview_window("main"); // keep Manager in use on all platforms
}
