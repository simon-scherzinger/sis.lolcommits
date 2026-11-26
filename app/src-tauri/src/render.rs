//! Meme captions, the blurred band for sharing, contact sheets – all in-process,
//! so the app needs neither ffmpeg nor ImageMagick.
use ab_glyph::{FontRef, PxScale};
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use std::path::Path;

pub const W: u32 = 1280;
pub const H: u32 = 720;

static FONT_BYTES: &[u8] = include_bytes!("../fonts/Anton-Regular.ttf");

fn font() -> FontRef<'static> {
    FontRef::try_from_slice(FONT_BYTES).expect("bundled font")
}

/// Scale to cover 1280x720 and crop the middle, like the CLI.
pub fn fit(img: RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let s = (W as f32 / w as f32).max(H as f32 / h as f32);
    let (nw, nh) = (((w as f32) * s).ceil() as u32, ((h as f32) * s).ceil() as u32);
    let scaled = imageops::resize(&img, nw.max(W), nh.max(H), imageops::FilterType::Triangle);
    imageops::crop_imm(&scaled, (scaled.width() - W) / 2, (scaled.height() - H) / 2, W, H).to_image()
}

/// Word-wrap so every line is at most `max_w` pixels wide at `size`, at most
/// `max` lines. If the text doesn't fit, the last line ends with "…".
fn wrap(text: &str, size: f32, max_w: i32, max: usize) -> Vec<String> {
    let fits = |s: &str| measure(size, s).0 <= max_w;
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() && lines.len() < max {
        let mut cur = words[i].to_string();
        i += 1;
        while i < words.len() && fits(&format!("{cur} {}", words[i])) {
            cur = format!("{cur} {}", words[i]);
            i += 1;
        }
        lines.push(cur);
    }
    let cut = i < words.len() || lines.last().is_some_and(|l| !fits(l));
    if let Some(last) = lines.last_mut().filter(|_| cut) {
        // drop words, then characters, until "line…" fits
        let mut t = last.clone();
        while !fits(&format!("{t}…")) {
            match t.rfind(' ') {
                Some(p) if p > 0 => t.truncate(p),
                _ => { t.pop(); if t.is_empty() { break; } }
            }
        }
        *last = format!("{}…", t.trim_end_matches(|c: char| c.is_whitespace() || ",.:;-".contains(c)));
    }
    lines
}

/// White text with a thick black outline.
fn outlined(img: &mut RgbImage, x: i32, y: i32, size: f32, stroke: i32, text: &str) {
    let f = font();
    let scale = PxScale::from(size);
    for dx in -stroke..=stroke {
        for dy in -stroke..=stroke {
            if dx * dx + dy * dy <= stroke * stroke + 1 && (dx, dy) != (0, 0) {
                draw_text_mut(img, Rgb([0, 0, 0]), x + dx, y + dy, scale, &f, text);
            }
        }
    }
    draw_text_mut(img, Rgb([255, 255, 255]), x, y, scale, &f, text);
}

fn measure(size: f32, text: &str) -> (i32, i32) {
    let (w, h) = text_size(PxScale::from(size), &font(), text);
    (w as i32, h as i32)
}

// Where caption() puts the commit hash and "repo · branch". Shared with
// label_rects(), so sharing can blur exactly these two labels.
const SHA_POS: (i32, i32) = (32, 22);
const SHA_SIZE: f32 = 52.0;
const REPO_TOP: i32 = 34;
const REPO_SIZE: f32 = 30.0;
const EDGE: i32 = 32;

pub fn caption(mut img: RgbImage, sha: &str, subject: &str, repo: &str, branch: &str) -> RgbImage {
    let (w, h) = (img.width() as i32, img.height() as i32);
    // commit message: big, centered, bottom
    let size = 72.0;
    let lines = wrap(subject, size, w - 2 * 48, 2);
    let line_h = (size * 1.12) as i32;
    let mut y = h - 40 - line_h * lines.len() as i32;
    for l in &lines {
        let (tw, _) = measure(size, l);
        outlined(&mut img, (w - tw) / 2, y, size, 4, l);
        y += line_h;
    }
    // sha top left, repo · branch top right
    outlined(&mut img, SHA_POS.0, SHA_POS.1, SHA_SIZE, 3, sha);
    let rb = format!("{repo} · {branch}");
    let (tw, _) = measure(REPO_SIZE, &rb);
    outlined(&mut img, w - tw - EDGE, REPO_TOP, REPO_SIZE, 2, &rb);
    img
}

/// Boxes (x, y, w, h) around the hash and the repo label as caption() drew
/// them, with room for the outline. Text height is taken generously from the
/// font size because glyphs reach above and below the measured box.
pub fn label_rects(width: u32, sha: &str, repo: &str, branch: &str) -> Vec<(u32, u32, u32, u32)> {
    let pad = 10;
    let (sw, _) = measure(SHA_SIZE, sha);
    let rb = format!("{repo} · {branch}");
    let (rw, _) = measure(REPO_SIZE, &rb);
    let w = width as i32;
    let rect = |x: i32, y: i32, tw: i32, size: f32| {
        let x0 = (x - pad).max(0);
        let y0 = (y - pad).max(0);
        let x1 = (x + tw + pad).min(w);
        let y1 = y + (size * 1.3) as i32 + pad;
        (x0 as u32, y0 as u32, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32)
    };
    vec![rect(SHA_POS.0, SHA_POS.1, sw, SHA_SIZE), rect(w - rw - EDGE, REPO_TOP, rw, REPO_SIZE)]
}

/// For sharing: blur only the given boxes (the hash and the repo label).
pub fn blur_rects(img: &RgbImage, rects: &[(u32, u32, u32, u32)]) -> RgbImage {
    let mut out = img.clone();
    for &(x, y, w, h) in rects {
        let (w, h) = (w.min(img.width().saturating_sub(x)), h.min(img.height().saturating_sub(y)));
        if w == 0 || h == 0 {
            continue;
        }
        let part = imageops::crop_imm(img, x, y, w, h).to_image();
        imageops::replace(&mut out, &imageops::blur(&part, 9.0), x as i64, y as i64);
    }
    out
}

/// JPEG without EXIF or comments.
pub fn save_jpeg(img: &RgbImage, path: &Path) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::new(f);
        JpegEncoder::new_with_quality(&mut w, 88).encode_image(img).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<RgbImage, String> {
    image::open(path).map(|i| i.to_rgb8()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Colour bars for tests without a camera (LOLCOMMITS_TEST_PATTERN=1).
pub fn test_pattern() -> RgbImage {
    let cols = [[230, 57, 70], [255, 214, 10], [42, 157, 143], [69, 123, 157], [155, 93, 229], [241, 91, 181]];
    let t = chrono::Local::now().timestamp_subsec_millis() as u32;
    RgbImage::from_fn(W, H, |x, y| {
        let c = cols[((x + t) / (W / 6) % 6) as usize];
        let shade = 0.75 + 0.25 * (y as f32 / H as f32);
        Rgb([(c[0] as f32 * shade) as u8, (c[1] as f32 * shade) as u8, (c[2] as f32 * shade) as u8])
    })
}

/// Contact sheet of up to 144 pictures, 320x180 tiles.
pub fn contact_sheet(files: &[String]) -> Result<RgbImage, String> {
    let n = files.len().min(144);
    if n == 0 {
        return Err("no pictures".into());
    }
    let mut cols = 1;
    while cols * cols < n {
        cols += 1;
    }
    let cols = cols.min(12) as u32;
    let rows = ((n as u32) + cols - 1) / cols;
    let (tw, th, pad, margin) = (320u32, 180u32, 4u32, 8u32);
    let mut sheet = RgbImage::from_pixel(margin * 2 + cols * tw + (cols - 1) * pad, margin * 2 + rows * th + (rows - 1) * pad, Rgb([0, 0, 0]));
    for (i, f) in files.iter().take(n).enumerate() {
        if let Ok(img) = load(Path::new(f)) {
            let t = imageops::resize(&fit(img), tw, th, imageops::FilterType::Triangle);
            let (c, r) = (i as u32 % cols, i as u32 / cols);
            imageops::replace(&mut sheet, &t, (margin + c * (tw + pad)) as i64, (margin + r * (th + pad)) as i64);
        }
    }
    Ok(sheet)
}

#[cfg(test)]
mod wrap_tests {
    use super::*;

    #[test]
    fn two_lines_at_most_with_ellipsis() {
        let max_w = W as i32 - 96;
        assert_eq!(wrap("fix typo", 72.0, max_w, 2), vec!["fix typo"]);
        let long = "feat(argocd): AppProject hector-backends und API-Account admincenter (ONE-6333) plus a lot more words that never fit into two lines at this size, really not";
        let lines = wrap(long, 72.0, max_w, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'), "{lines:?}");
        assert!(lines.iter().all(|l| measure(72.0, l).0 <= max_w));
        let word = wrap(&"x".repeat(200), 72.0, max_w, 2);
        assert!(word.len() == 1 && word[0].ends_with('…') && measure(72.0, &word[0]).0 <= max_w);
        // LOLCOMMITS_RENDER_SAMPLES=dir writes example pictures to look at
        if let Ok(dir) = std::env::var("LOLCOMMITS_RENDER_SAMPLES") {
            for (i, s) in ["fix typo", "feat(argocd): AppProject hector-backends und API-Account admincenter (ONE-6333)", long].iter().enumerate() {
                let img = caption(test_pattern(), "f520692079", s, "dsb.argocd", "feat/ONE-6333-argocd-admincenter-api");
                save_jpeg(&img, &Path::new(&dir).join(format!("sample-{i}.jpg"))).unwrap();
            }
        }
    }
}
