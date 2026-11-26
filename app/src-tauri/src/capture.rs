//! Camera access. The app owns the camera permission, so macOS asks once –
//! not once per terminal, IDE and git client like the hook-only CLI.
use crate::{config, render};
use image::RgbImage;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use serde::Serialize;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One capture at a time – the camera can't be opened twice.
pub static CAMERA: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
pub struct Device {
    pub value: String,
    pub label: String,
}

pub fn test_mode() -> bool {
    std::env::var("LOLCOMMITS_TEST_PATTERN").map(|v| v == "1").unwrap_or(false)
}

/// Ask macOS for camera access (shows the system prompt the first time).
pub fn request_permission() -> bool {
    if test_mode() {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        if nokhwa::nokhwa_check() {
            return true;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        nokhwa::nokhwa_initialize(move |ok| {
            let _ = tx.send(ok);
        });
        rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Remembered "yes" of permission_granted(). Every photo asks macOS again
/// (grab_native), so a permission taken back in System Settings gets the
/// clear message instead of a bare camera error.
#[cfg(target_os = "macos")]
static GRANTED: AtomicBool = AtomicBool::new(false);

pub fn permission_granted() -> bool {
    if test_mode() {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        // ~50 ms per check and asked on every window refresh. A "yes" is
        // remembered; a "no" is asked again, so allowing it later shows up.
        if GRANTED.load(Ordering::Relaxed) {
            return true;
        }
        let ok = nokhwa::nokhwa_check();
        GRANTED.store(ok, Ordering::Relaxed);
        ok
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn devices() -> Vec<Device> {
    nokhwa::query(ApiBackend::Auto)
        .unwrap_or_default()
        .into_iter()
        .map(|c| Device {
            value: match c.index() {
                CameraIndex::Index(i) => i.to_string(),
                CameraIndex::String(s) => s.clone(),
            },
            label: c.human_name(),
        })
        .collect()
}

fn index_from_config() -> CameraIndex {
    let dev = config::get("device").unwrap_or_default();
    let digits: String = dev.chars().filter(|c| c.is_ascii_digit()).collect(); // "0" or "/dev/video2"
    CameraIndex::Index(digits.parse().unwrap_or(0))
}

fn delay() -> f32 {
    config::get("delay").and_then(|d| d.parse().ok()).unwrap_or(1.5f32).clamp(0.0, 10.0)
}

/// Grab one frame after the warm-up delay. Falls back to ffmpeg if the native
/// camera API fails and ffmpeg happens to be installed.
pub fn grab() -> Result<RgbImage, String> {
    let _lock = CAMERA.lock().unwrap_or_else(|e| e.into_inner());
    if test_mode() {
        std::thread::sleep(Duration::from_millis(600));
        return Ok(render::test_pattern());
    }
    match grab_native() {
        Ok(img) => Ok(img),
        Err(e) => grab_ffmpeg().map_err(|e2| format!("{e} (ffmpeg fallback: {e2})")),
    }
}

fn grab_native() -> Result<RgbImage, String> {
    #[cfg(target_os = "macos")]
    GRANTED.store(false, Ordering::Relaxed);
    if !permission_granted() {
        return Err("no camera permission – allow lolcommits in System Settings › Privacy & Security › Camera".into());
    }
    let fmt = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
    let mut cam = Camera::new(index_from_config(), fmt).map_err(|e| format!("camera: {e}"))?;
    let _unlock = Unlock(cam.info().misc());
    cam.open_stream().map_err(|e| format!("camera: {e}"))?;
    let until = Instant::now() + Duration::from_secs_f32(delay());
    let mut frame = cam.frame().map_err(|e| format!("camera: {e}"))?;
    while Instant::now() < until {
        if let Ok(f) = cam.frame() {
            frame = f;
        }
    }
    let _ = cam.stop_stream();
    frame.decode_image::<RgbFormat>().map_err(|e| format!("decode: {e}"))
}

/// nokhwa 0.10 locks the camera for configuration when it opens the stream and
/// never unlocks it (its `locked` flag is never set), so Photo Booth, FaceTime
/// or WhatsApp get no picture until lolcommits quits. Unlock it ourselves –
/// also when grabbing fails halfway.
struct Unlock(String);

#[allow(unexpected_cfgs)] // objc 0.2 macros check cfg(feature = "cargo-clippy")
impl Drop for Unlock {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            use objc::runtime::Object;
            use objc::{class, msg_send, sel, sel_impl};
            let id: *mut Object = msg_send![class!(NSString), alloc];
            let id: *mut Object = msg_send![id, initWithBytes: self.0.as_ptr() length: self.0.len() encoding: 4usize];
            if id.is_null() {
                return;
            }
            let dev: *mut Object = msg_send![class!(AVCaptureDevice), deviceWithUniqueID: id];
            if !dev.is_null() {
                let _: () = msg_send![dev, unlockForConfiguration];
            }
            let _: () = msg_send![id, release];
        }
    }
}

fn grab_ffmpeg() -> Result<RgbImage, String> {
    let ff = config::which("ffmpeg").ok_or("ffmpeg not installed")?;
    let tmp = std::env::temp_dir().join(format!("lolcommits-{}.jpg", std::process::id()));
    let dev = config::get("device").unwrap_or_default();
    let mut c = config::cmd(ff.to_str().unwrap());
    c.args(["-hide_banner", "-loglevel", "error", "-y"]);
    if cfg!(target_os = "macos") {
        let d = if dev.is_empty() { "0".to_string() } else { dev };
        c.args(["-f", "avfoundation", "-pixel_format", "nv12", "-framerate", "30", "-video_size", "1280x720", "-i", &format!("{d}:none")]);
    } else {
        let d = if dev.is_empty() { "/dev/video0".to_string() } else { dev };
        c.args(["-f", "v4l2", "-video_size", "1280x720", "-i", &d]);
    }
    c.args(["-ss", &delay().to_string(), "-frames:v", "1"]).arg(&tmp);
    let out = c.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("failed").to_string());
    }
    let img = render::load(&tmp);
    let _ = std::fs::remove_file(&tmp);
    img
}
