//! End-to-end tests of the app logic against a temporary home: fake Google
//! Drive for desktop folder, own git config, test pattern instead of the camera.
use crate::store::Shot;
use crate::{capture, config, drive, render, share, store};
use std::fs;
use std::path::PathBuf;

fn sandbox() -> PathBuf {
    let root = std::env::temp_dir().join(format!("lolcommits-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("cloud/GoogleDrive-kim@example.com/Geteilte Ablagen/Team Fun")).unwrap();
    fs::create_dir_all(root.join("cloud/GoogleDrive-kim@example.com/Meine Ablage/private")).unwrap();
    std::env::set_var("LOLCOMMITS_DIR", root.join("lol"));
    std::env::set_var("GIT_CONFIG_GLOBAL", root.join("gitconfig"));
    std::env::set_var("LOLCOMMITS_TEST_PATTERN", "1");
    std::env::set_var("LOLCOMMITS_CLOUDSTORAGE", root.join("cloud"));
    root
}

fn shoot(subject: &str, stem: &str) -> Shot {
    let img = render::caption(render::fit(capture::grab().unwrap()), "abc1234567", subject, "repo", "main");
    let out = config::lol_dir().join("repo").join(format!("{stem}.jpg"));
    render::save_jpeg(&img, &out).unwrap();
    let shot = Shot {
        ts: "2026-11-25T10:00:00+01:00".into(),
        repo: "repo".into(),
        branch: "main".into(),
        sha: "abc1234567".into(),
        subject: subject.into(),
        file: out.to_string_lossy().into(),
        shared: false,
        private: false,
    };
    store::append(&shot).unwrap();
    shot
}

#[test]
fn full_flow() {
    let root = sandbox();

    // Drive for desktop: only shared drives are offered, not "Meine Ablage"
    let folders = drive::folders();
    assert_eq!(folders.len(), 1, "{:?}", folders.iter().map(|f| &f.name).collect::<Vec<_>>());
    assert_eq!(folders[0].name, "Team Fun");
    assert_eq!(folders[0].account, "kim@example.com");

    // capture → local store, no EXIF
    let a = shoot("first: tabs\tand stuff", "20261125-100000-abc1234567");
    let b = shoot("second", "20261125-100100-abc1234567");
    assert_eq!(store::shots().len(), 2);
    let bytes = fs::read(&a.file).unwrap();
    assert!(!bytes.windows(4).any(|w| w == b"Exif"));

    // connect the shared drive folder, umlaut name
    let target = PathBuf::from(&folders[0].path).join("lolcommits");
    fs::create_dir_all(&target).unwrap();
    share::probe(&share::Target::Folder(target.clone())).unwrap();
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "probe cleans up");
    config::set("share.target", target.to_str().unwrap()).unwrap();
    config::set("share.name", "Jürgen Groß").unwrap();
    assert!(share::configured());

    // repo not asked yet → other-repos, repo label blurred, repo hidden in the sidecar
    assert_eq!(share::repo_choice("repo"), None);
    share::upload(&a).unwrap();
    assert!(share::upload(&a).is_err(), "no double share");
    let other: Vec<_> = fs::read_dir(target.join("other-repos")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(other.len(), 2, "jpg + sidecar: {other:?}");
    assert!(other.iter().all(|f| f.starts_with("20261125-juergen-gross-")), "{other:?}");
    let side = other.iter().find(|f| f.ends_with(".tsv")).unwrap();
    let meta = fs::read_to_string(target.join("other-repos").join(side)).unwrap();
    assert_eq!(meta, "2026-11-25\tJürgen Groß\t-\tfirst: tabs and stuff\n");

    // repo allowed by name → its own folder, not blurred
    share::set_repo_choice("repo", true).unwrap();
    share::upload(&b).unwrap();
    let in_repo = fs::read_dir(target.join("repo")).unwrap().count();
    assert_eq!(in_repo, 2);
    let team = share::team().unwrap();
    assert_eq!(team.len(), 2);
    assert!(team.iter().all(|r| r.mine && r.name == "Jürgen Groß"));
    assert_eq!(team.iter().filter(|r| r.repo == "repo").count(), 1);
    assert!(store::shots().iter().all(|s| s.shared));

    // same day: the most recently shared picture comes first, not the folder order ("repo" > "other-repos")
    let first_jpg = target.join("other-repos").join(other.iter().find(|f| f.ends_with(".jpg")).unwrap());
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    fs::File::options().write(true).open(&first_jpg).unwrap().set_modified(later).unwrap();
    let order: Vec<_> = share::team().unwrap().into_iter().map(|r| r.subject).collect();
    assert_eq!(order, ["first: tabs and stuff", "second"]);

    // a teammate's picture in the same folder is not mine and survives my unshare-all
    fs::write(target.join("repo/20261124-kim-0000abcd.jpg"), fs::read(&a.file).unwrap()).unwrap();
    fs::write(target.join("repo/20261124-kim-0000abcd.tsv"), "2026-11-24\tKim\trepo\tship it\n").unwrap();
    assert_eq!(share::team().unwrap().iter().filter(|r| !r.mine).count(), 1);

    // no expiry by default: an old picture of mine survives the next upload
    fs::write(target.join("repo/20200101-juergen-gross-00000001.jpg"), b"x").unwrap();
    fs::write(target.join("repo/20200101-juergen-gross-00000001.tsv"), "2020-01-01\tJürgen Groß\trepo\told\n").unwrap();
    let c = shoot("third", "20261125-100200-abc1234567");
    share::upload(&c).unwrap();
    assert!(target.join("repo/20200101-juergen-gross-00000001.jpg").exists(), "default must not expire shared pictures");

    // with an expiry set it is removed (only mine)
    config::set("share.retention", "30").unwrap();
    let d = shoot("fourth", "20261125-100300-abc1234567");
    share::upload(&d).unwrap();
    assert!(!target.join("repo/20200101-juergen-gross-00000001.jpg").exists());
    assert!(!target.join("repo/20200101-juergen-gross-00000001.tsv").exists());
    assert!(target.join("repo/20261124-kim-0000abcd.jpg").exists());
    config::set("share.retention", "").unwrap();

    // sharing blurs only the two labels when the repo is hidden
    share::set_repo_choice("repo", false).unwrap();
    let prepared = render::load(&share::prepare(&c).unwrap()).unwrap();
    let original = render::load(std::path::Path::new(&c.file)).unwrap();
    let rects = render::label_rects(original.width(), &c.sha, &c.repo, &c.branch);
    assert!(rects.iter().all(|r| r.1 + r.3 < 120), "labels sit at the top: {rects:?}");
    let diff = |x: u32, y: u32| {
        let (p, o) = (prepared.get_pixel(x, y).0, original.get_pixel(x, y).0);
        (0..3).map(|i| (p[i] as i32 - o[i] as i32).abs()).max().unwrap()
    };
    assert!(diff(640, 300) <= 12, "centre must stay sharp (jpeg noise only)");
    assert!(diff(640, 40) <= 12, "top centre between the labels must stay sharp");
    if let Ok(out) = std::env::var("LOLCOMMITS_SAMPLE_OUT") {
        let _ = fs::copy(share::prepare(&c).unwrap(), out);
    }

    // unshare one: jpg and sidecar go, empty folder too
    share::unshare(&a.file).unwrap();
    assert!(!target.join("other-repos").exists());
    assert!(!store::find(&a.file).unwrap().shared);

    // unshare all removes only my pictures
    share::unshare_all().unwrap();
    let left: Vec<_> = fs::read_dir(target.join("repo")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(left.len(), 2, "only Kim's picture is left: {left:?}");
    assert!(store::shared_rows().is_empty());

    // review: swiping left keeps a picture private, undo brings it back
    store::set_private(&b.file, true).unwrap();
    assert!(store::find(&b.file).unwrap().private);
    store::set_private(&b.file, false).unwrap();
    assert!(!store::find(&b.file).unwrap().private);
    store::set_private(&b.file, true).unwrap();
    store::remove(&b.file).unwrap();
    assert!(store::private_files().is_empty(), "deleting a picture drops it from the private list");

    // contact sheet, then forget everything
    let files: Vec<String> = store::shots().into_iter().map(|s| s.file).collect();
    let sheet = render::contact_sheet(&files).unwrap();
    assert!(sheet.width() > 320);
    store::forget_all();
    assert!(store::shots().is_empty());

    // lolcommits.folders: several values, replaced as a whole
    config::set_all("folders", &["/work".into(), "~/Developer".into()]).unwrap();
    assert_eq!(config::get_all("folders"), vec!["/work", "~/Developer"]);
    config::set_all("folders", &["/work".into()]).unwrap();
    assert_eq!(config::get_all("folders"), vec!["/work"]);
    config::set_all("folders", &[]).unwrap();
    assert!(config::get_all("folders").is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn reads_the_token_from_rclone_authorize() {
    let out = "Paste the following into your remote machine --->\n{\"access_token\":\"a\",\"token_type\":\"Bearer\",\"refresh_token\":\"r\",\"expiry\":\"2026-11-26T09:00:00+01:00\"}\n<---End paste\n";
    assert_eq!(drive::token_from(out).unwrap(), r#"{"access_token":"a","token_type":"Bearer","refresh_token":"r","expiry":"2026-11-26T09:00:00+01:00"}"#);
    assert!(drive::token_from("Failed to get token").is_none());
    assert_eq!(drive::target_for("0AB"), "rclone:lolcommits-gdrive,team_drive=0AB:lolcommits");
}

#[test]
fn parses_rclone_drives() {
    let d = drive::parse_drives(r#"[{"id":"0AB","kind":"drive#teamDrive","name":"lolcommits"},{"id":"0CD","kind":"drive#teamDrive","name":"Eng"}]"#).unwrap();
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].name, "lolcommits");
    assert!(drive::parse_drives("not json").is_err());
}

#[test]
fn repo_folder_names_are_safe() {
    assert_eq!(share::repo_folder("sis.lolcommits"), "sis.lolcommits");
    assert_eq!(share::repo_folder(".hidden"), "hidden");
    assert_eq!(share::repo_folder("a/b c"), "a-b-c");
    assert_eq!(share::repo_folder("other-repos"), "repo");
}

#[test]
fn slugs_match_the_cli() {
    assert_eq!(share::slug("Jürgen Groß"), "juergen-gross");
    assert_eq!(share::slug("Ä Person"), "ae-person");
    assert_eq!(share::slug("  --Kim!! "), "kim");
}
