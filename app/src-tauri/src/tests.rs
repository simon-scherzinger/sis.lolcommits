//! End-to-end tests of the app logic against a temporary home: fake Google
//! Drive for desktop folder, own git config, test pattern instead of the camera.
use crate::store::Shot;
use crate::{capture, config, drive, render, share, store, team};
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
    share::upload(&a, false).unwrap();
    assert!(share::upload(&a, false).is_err(), "no double share");
    let other: Vec<_> = fs::read_dir(target.join("other-repos")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(other.len(), 1, "only the picture, the sidecar is in .meta/: {other:?}");
    assert!(other[0].starts_with("20261125-juergen-gross-") && other[0].ends_with(".jpg"), "{other:?}");
    let side = other[0].replace(".jpg", ".tsv");
    let meta = fs::read_to_string(target.join(".meta/other-repos").join(&side)).unwrap();
    assert_eq!(meta, "2026-11-25\tJürgen Groß\t-\tfirst: tabs and stuff\n");

    // repo allowed by name → its own folder, not blurred
    share::set_repo_choice("repo", true).unwrap();
    share::upload(&b, false).unwrap();
    let in_repo = fs::read_dir(target.join("repo")).unwrap().count();
    assert_eq!(in_repo, 1);
    assert_eq!(fs::read_dir(target.join(".meta/repo")).unwrap().count(), 1);
    let team = share::team().unwrap();
    assert_eq!(team.len(), 2);
    assert!(team.iter().all(|r| r.mine && r.name == "Jürgen Groß"));
    assert_eq!(team.iter().filter(|r| r.repo == "repo").count(), 1);
    assert!(store::shots().iter().all(|s| s.shared));

    // same day: the most recently shared picture comes first, not the folder order ("repo" > "other-repos")
    let first_jpg = target.join("other-repos").join(&other[0]);
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    fs::File::options().write(true).open(&first_jpg).unwrap().set_modified(later).unwrap();
    let order: Vec<_> = share::team().unwrap().into_iter().map(|r| r.subject).collect();
    assert_eq!(order, ["first: tabs and stuff", "second"]);

    // a teammate's picture in the same folder is not mine and survives my unshare-all;
    // shared by an older version, so its sidecar is still next to the picture
    fs::write(target.join("repo/20261124-kim-0000abcd.jpg"), fs::read(&a.file).unwrap()).unwrap();
    fs::write(target.join("repo/20261124-kim-0000abcd.tsv"), "2026-11-24\tKim\trepo\tship it\n").unwrap();
    assert_eq!(share::team().unwrap().iter().filter(|r| !r.mine).count(), 1);

    // my own old-style sidecars move into .meta/ (not the teammate's), the picture still shows
    fs::write(target.join("repo/20201231-juergen-gross-00000002.jpg"), fs::read(&a.file).unwrap()).unwrap();
    fs::write(target.join("repo/20201231-juergen-gross-00000002.tsv"), "2020-12-31\tJürgen Groß\trepo\tlegacy\n").unwrap();
    assert_eq!(share::move_old_sidecars().unwrap(), 1);
    assert!(target.join(".meta/repo/20201231-juergen-gross-00000002.tsv").is_file());
    assert!(!target.join("repo/20201231-juergen-gross-00000002.tsv").exists());
    assert!(target.join("repo/20261124-kim-0000abcd.tsv").is_file());
    assert_eq!(share::move_old_sidecars().unwrap(), 0, "nothing left to move");
    assert!(share::team().unwrap().iter().any(|r| r.subject == "legacy"));
    assert!(!share::team().unwrap().iter().any(|r| r.file.contains(".meta")), ".meta/ is not a repo folder");

    // no expiry by default: an old picture of mine survives the next upload
    fs::write(target.join("repo/20200101-juergen-gross-00000001.jpg"), b"x").unwrap();
    fs::write(target.join("repo/20200101-juergen-gross-00000001.tsv"), "2020-01-01\tJürgen Groß\trepo\told\n").unwrap();
    let c = shoot("third", "20261125-100200-abc1234567");
    share::upload(&c, false).unwrap();
    assert!(target.join("repo/20200101-juergen-gross-00000001.jpg").exists(), "default must not expire shared pictures");

    // with an expiry set it is removed (only mine)
    config::set("share.retention", "30").unwrap();
    let d = shoot("fourth", "20261125-100300-abc1234567");
    share::upload(&d, false).unwrap();
    assert!(!target.join("repo/20200101-juergen-gross-00000001.jpg").exists());
    assert!(!target.join("repo/20200101-juergen-gross-00000001.tsv").exists(), "old-style sidecar goes too");
    assert!(!target.join("repo/20201231-juergen-gross-00000002.jpg").exists());
    assert!(!target.join(".meta/repo/20201231-juergen-gross-00000002.tsv").exists(), "sidecar in .meta/ goes too");
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
    assert!(!target.join(".meta/other-repos").exists());
    assert!(!store::find(&a.file).unwrap().shared);

    // unshare all removes only my pictures
    share::unshare_all().unwrap();
    let left: Vec<_> = fs::read_dir(target.join("repo")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(left.len(), 2, "only Kim's picture is left: {left:?}");
    assert!(!target.join(".meta/repo").exists(), "my sidecars in .meta/ are gone too");
    assert!(store::shared_rows().is_empty());

    // Settings › Repos › Share off: out of Review, can't be shared, back when turned on again
    let before = store::to_review(&store::shots());
    assert!(before > 0);
    config::set_listed(share::NEVER, "repo", true).unwrap();
    assert!(!share::may_share("repo") && share::may_share("other"));
    assert_eq!(store::to_review(&store::shots()), 0);
    assert!(share::upload(&b, false).unwrap_err().contains("sharing is off"));
    config::set_listed(share::NEVER, "repo", false).unwrap();
    assert_eq!(store::to_review(&store::shots()), before);

    // Settings › Repos › Photos off: a list of repo names, other values untouched
    config::set_listed(share::SKIP, "client.app", true).unwrap();
    config::set_listed(share::SKIP, "repo", true).unwrap();
    config::set_listed(share::SKIP, "repo", true).unwrap();
    assert_eq!(config::get_all(share::SKIP), vec!["client.app", "repo"], "no duplicates");
    assert!(!share::photos_on("repo") && !share::photos_on("client.app") && share::photos_on("client-app"));
    config::set_listed(share::SKIP, "repo", false).unwrap();
    assert_eq!(config::get_all(share::SKIP), vec!["client.app"]);
    config::set_listed(share::SKIP, "client.app", false).unwrap();

    // team settings: one file per repo in .meta/repos/, a hidden name only as a hash
    share::set_repo_choice("acme-shop", false).unwrap();
    share::set_repo_choice("repo", true).unwrap();
    let team_files = || -> String {
        fs::read_dir(target.join(".meta/repos")).unwrap().flatten()
            .map(|e| e.file_name().to_string_lossy().to_string() + ":" + &fs::read_to_string(e.path()).unwrap()).collect()
    };
    assert!(!team_files().contains("acme-shop"), "{}", team_files());
    assert!(team_files().contains("show\trepo"), "shown names stay readable: {}", team_files());
    assert_eq!(share::repo_choice("acme-shop"), Some(false));
    // the first answer becomes the team's; a later one doesn't overrule it
    assert!(share::answer_repo_choice("acme-shop", true).unwrap() == false, "the team hides it");
    assert_eq!(share::repo_choice("acme-shop"), Some(false));
    assert!(share::answer_repo_choice("new-repo", true).unwrap());
    assert_eq!(team::fresh_repo_choice("new-repo").unwrap(), Some(true));
    // a Drive conflict copy from two answers at once: hide wins
    let new_file = fs::read_dir(target.join(".meta/repos")).unwrap().flatten()
        .find(|e| fs::read_to_string(e.path()).unwrap() == "show\tnew-repo\n").unwrap().path();
    let copy = new_file.with_file_name(format!("{} (1)", new_file.file_name().unwrap().to_string_lossy()));
    fs::write(&copy, "hide\n").unwrap();
    assert_eq!(share::repo_choice("new-repo"), Some(false));
    share::set_repo_choice("new-repo", true).unwrap();
    assert!(!copy.exists(), "a deliberate choice clears conflict copies");
    assert_eq!(share::repo_choice("new-repo"), Some(true));
    // changing it in Settings changes it for everyone
    share::set_repo_choice("new-repo", false).unwrap();
    assert_eq!(team::fresh_repo_choice("new-repo").unwrap(), Some(false));

    // choices from before team settings: published once, the stricter one wins
    let names = config::lol_dir().join("repo-names.tsv");
    let before_names = fs::read_to_string(&names).unwrap();
    fs::write(&names, before_names.clone() + "only-mine\tshow\nconflict\tshow\n").unwrap();
    team::set_repo_choice("conflict", false).unwrap();
    team::set_repo_choice("repo", false).unwrap(); // a teammate hid it meanwhile; locally it is shown
    let n = team::publish_local_choices().unwrap();
    assert_eq!(team::fresh_repo_choice("only-mine").unwrap(), Some(true));
    assert_eq!(team::fresh_repo_choice("conflict").unwrap(), Some(false), "hide wins");
    assert_eq!(team::fresh_repo_choice("repo").unwrap(), Some(false), "hide wins");
    assert_eq!(n, 1, "only the repo the team didn't know");
    assert_eq!(team::publish_local_choices().unwrap(), 0, "once per team gallery");
    // the team's choice wins over this computer's, also for the upload
    assert_eq!(share::repo_choice("repo"), Some(false));
    let e = shoot("team hides it", "20261125-100500-abc1234567");
    share::upload(&e, false).unwrap();
    assert!(store::shared_rows().iter().any(|(l, r)| l == &e.file && r.starts_with("other-repos/")), "{:?}", store::shared_rows());
    share::unshare(&e.file).unwrap();
    share::set_repo_choice("repo", true).unwrap();

    // retention: the team's value is a maximum
    config::set("share.retention", "").unwrap();
    team::set_retention(Some(30)).unwrap();
    assert_eq!(share::retention_days(), 30);
    config::set("share.retention", "60").unwrap();
    assert_eq!(share::retention_days(), 30, "can't keep longer than the team");
    config::set("share.retention", "10").unwrap();
    assert_eq!(share::retention_days(), 10, "may delete earlier");
    team::set_retention(None).unwrap();
    assert!(!target.join(".meta/retention").exists());
    assert_eq!(share::retention_days(), 10);
    config::set("share.retention", "").unwrap();

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

    // settings are cached, but a change by the command line tool (same git
    // config, written behind the app's back) is seen at the next read
    assert_eq!(config::get("share.name").as_deref(), Some("Jürgen Groß"));
    let cli = |args: &[&str]| assert!(std::process::Command::new("git").args(["config", "--global"]).args(args).status().unwrap().success());
    cli(&["lolcommits.share.name", "Kim from the CLI"]);
    assert_eq!(config::get("share.name").as_deref(), Some("Kim from the CLI"));
    cli(&["--add", "lolcommits.folders", "/from-cli"]);
    assert_eq!(config::get_all("folders"), vec!["/from-cli"]);
    cli(&["--unset-all", "lolcommits.folders"]);
    assert!(config::get_all("folders").is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn outdated_google_sign_in() {
    // remote without client_id: rclone's shared client, retired by Google in 2026
    assert!(drive::outdated("", Some("ours.apps.googleusercontent.com")));
    assert!(drive::outdated("", None), "even without a client to switch to");
    // made with another client than the one in use now
    assert!(drive::outdated("old.apps.googleusercontent.com", Some("ours.apps.googleusercontent.com")));
    assert!(!drive::outdated("ours.apps.googleusercontent.com", Some("ours.apps.googleusercontent.com")));
    // own client, but this build has none to compare with: nothing to renew
    assert!(!drive::outdated("ours.apps.googleusercontent.com", None));
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
