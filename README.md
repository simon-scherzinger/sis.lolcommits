# lolcommits (2027 edition)

A webcam selfie for every git commit, with the commit message on it in big meme letters.
Remake of the old [lolcommits](https://github.com/lolcommits/lolcommits) Ruby gem as a menu bar app for macOS (and Linux).

- **Download, open, done:** a setup assistant handles the camera, the git hook and the team gallery. No terminal, no Homebrew.
- **Lives in the menu bar:** the icon flashes when it takes a photo. From the menu you can pause (1 hour, until tomorrow, until you resume), take a test photo, or review your new photos.
- **Doesn't slow down commits:** `git commit` returns immediately and the app takes the photo in the background.
- **One camera permission:** the app owns the camera, so your terminal, IDE and git client don't each ask for it.
- **Hot or not:** new photos wait in a review stack. Swipe right to share one with your team, left to keep it to yourself. Undo is ⌘Z.
- **Team gallery, privacy first:** a Google shared drive with one folder per repo. Client repos stay anonymous, and one click takes everything back ([details](#team-gallery)).
- **Extras:** gallery with search, timelapse video, year-in-review contact sheet ("wrapped").
- **Updates:** the app looks for new releases every few hours and installs one when you click "Update and restart" in the menu.

## Install

1. Download the `.dmg` from the [latest release](https://github.com/simon-scherzinger/sis.lolcommits/releases) and move lolcommits to Applications.
2. Open it. The assistant walks you through:
   - **Fair use:** what stays on your Mac and what is shared (GDPR versus team spirit, in short)
   - **Camera:** allow it once and take a test photo
   - **On:** take a photo on every commit, in all repos (needs git 2.54+, which ships with current macOS)
   - **Team gallery (optional):** pick a shared drive from Google Drive for desktop, or sign in with Google
3. Commit something.

On **Linux** use the `.AppImage` or `.deb` from the release.
Without Google Drive for desktop, "Sign in with Google" works through [rclone](https://rclone.org) (the app offers to install it) and needs a Google client of your company ([details](#google-sign-in-without-drive-for-desktop)).

**With Claude Code:** tell it

> Install lolcommits following https://github.com/simon-scherzinger/sis.lolcommits/blob/main/INSTALL.md

## Team gallery

Pictures stay on your computer unless you share them. Sharing is opt-in per person.

**Setup for the team (once):** create a Google shared drive, e.g. "lolcommits", and add the team's Google group.
Use a shared drive rather than someone's My Drive, so the pictures belong to the company and not to one person.

**Setup for each person:** in the app, open Team › Set up team gallery and pick the shared drive.
- **With Google Drive for desktop:** its shared drives show up right away, with no sign-in.
- **Without it:** "Sign in with Google" opens the browser, then you pick the shared drive. This needs a Google client of your company, see below.

**Review:** new photos land in the Review tab (the menu bar shows "Review N new photos…").
Each card shows the picture exactly as it would be uploaded.
- **Swipe right / → / ♥:** shares it.
- **Swipe left / ← / ✕:** keeps it on your computer, and it won't come up again.
- **⌘Z:** undoes the last swipe.

**Sorted by repo:** the first time anyone in the team shares from a repo, the app asks whether the team may see its name.
- **Yes:** pictures go to `<repo>/` and the repo label stays readable.
- **No:** they go to `other-repos/` with the repo label and commit hash blurred.

The answer is a team setting: it is stored in the shared drive and counts for everyone, so a client repo one person hides never shows up by name through someone else. You can change it per repo in Settings › Repos; that changes it for the whole team and applies to pictures shared after that.

**What gets uploaded:**
- the picture without metadata, with the hash and repo label blurred unless the repo is shown by name
- one small sidecar file per picture in the hidden `.meta/` folder: the date (no time of day), your name, the repo (if shown) and the commit message

Files are named `<date>-<person>-<random>`, so several people can share into the same folder without conflicts.
Shared pictures stay until you take them back, unless you set an expiry in Settings. The team can also set a limit there ("Team: shared pictures stay at most"): nobody keeps them longer, anyone may remove theirs earlier.

**Team settings** live in the hidden `.meta/` folder of the team gallery, one small file each, so two people changing something at once don't create Drive conflict copies:
- `.meta/repos/<sha256 of the repo name>`: `show<TAB><name>` or `hide`. A hidden repo appears only as a hash. That keeps the name out of sight, but someone with access to the drive who guesses a name can check it against the hashes. If two people answer at the same time and Drive keeps both files, "hide" wins.
- `.meta/retention`: the team's limit in days.
Your team should agree on a period, see [PRIVACY.md](PRIVACY.md).

### Google sign-in without Drive for desktop

Google Drive for desktop is the easiest way: no sign-in in lolcommits, nothing to set up.

Without it, lolcommits signs in to Google Drive through rclone with an OAuth client of your company.
rclone's own shared client no longer works (Google retired it in 2026).
- **entrecode:** the app from the releases has entrecode's client built in. Just sign in.
- **Other teams:** a Google Workspace admin creates a client once and shares ID and secret with the team. Everyone enters them in the setup ("Use our own client…").

Creating the client (about 10 minutes, in the [Google Cloud console](https://console.cloud.google.com)):
1. Create a project, e.g. "lolcommits", in your organization.
2. APIs & Services › Library: enable the **Google Drive API**.
3. Google Auth Platform › Audience: user type **Internal**. Only accounts of your organization can sign in, and Google doesn't need to review the app.
4. Google Auth Platform › Clients › Create client: type **Desktop app**, name "lolcommits". Copy the client ID and secret.

The secret of a desktop client isn't confidential ([Google](https://developers.google.com/identity/protocols/oauth2/native-app)), so sharing it in the team chat is fine. With "Internal" it only works for your organization.

### Privacy

Faces plus commit times are personal data, and in a company they are employee data.
lolcommits is built so that the team gallery stays fun and doesn't turn into monitoring:

- opt-in per person, every picture reviewed exactly as uploaded, nicknames allowed
- withdrawal is one click (Privacy › Unshare all)
- no metadata, repo names hidden unless you allow them per repo
- dates only, no times, no statistics per person
- no face or emotion recognition, ever

Before rolling it out in a company, read [PRIVACY.md](PRIVACY.md). It has a checklist and a notice template for your team.

## Command line

The app bundles the `lolcommits` script. Settings › App › "Install `lolcommits` command" puts it on your PATH.
It also works on its own, without the app, on macOS and Linux with `ffmpeg` (plus `imagemagick` on macOS for captions).

```bash
lolcommits enable --global   # every repo (git 2.54+)
lolcommits enable            # just this repo (any git version)
lolcommits disable [--global]
lolcommits status | doctor

lolcommits last              # open the latest shot
lolcommits gallery           # HTML gallery
lolcommits timelapse [repo]  # mp4 (needs ffmpeg)
lolcommits wrapped [year]    # contact sheet of the year

lolcommits share [file]      # preview, then upload
lolcommits unshare [file|--all]
lolcommits forget <file|last|--all>
lolcommits ui                # open the app
```

- **Skip one commit:** `LOLCOMMITS_SKIP=1 git commit …`
- **Pause in one repo:** `git config lolcommits.enabled false`
- **Only work repos:** `git config --global --add lolcommits.folders ~/Developer` takes photos only in repos below that folder (add several; none set: every repo). In the app: Settings › Capture › Only in these folders.
- **With the app installed:** the hook only queues a job in `~/.lolcommits/queue/`. The app takes the photo.

### Config

Everything is plain git config, shared by the app and the command line:

| key | default | |
|---|---|---|
| `lolcommits.device` | `0` (macOS) / `/dev/video0` (Linux) | camera |
| `lolcommits.delay` | `1.5` | seconds of camera warm-up |
| `lolcommits.dir` | `~/.lolcommits` | where pictures go |
| `lolcommits.enabled` | `true` | `false` pauses without removing the hook |
| `lolcommits.folders` | – (all) | photos only in repos below these folders, multi-valued (`--add`) |
| `lolcommits.skipRepo` | – | repo names without photos, in every clone; multi-valued (`--add`), Settings › Repos › Photos |
| `lolcommits.pausedUntil` | – | unix time, set by "Pause" in the menu bar |
| `lolcommits.retention` | keep | delete local pictures after N days |
| `lolcommits.share.target` | off | a folder (Drive for desktop, NAS) or `rclone:<remote>:<path>` |
| `lolcommits.share.name` | – | your name in the gallery, also the folder name |
| `lolcommits.share.retention` | `0` (keep) | delete your shared pictures after N days |
| `lolcommits.share.googleClientId` / `…Secret` | – | your team's Google OAuth client for "Sign in with Google" |
| `lolcommits.share.auto` | `false` | upload every photo without review (repos not asked yet stay hidden) |
| `lolcommits.neverShare` | – | repo names whose photos are never shared or reviewed; multi-valued (`--add`), Settings › Repos › Share |
| `lolcommits.update.auto` | `true` | look for app updates on GitHub every few hours |
| `lolcommits.remind` | `true` | once a day, from 10:00, a notification when photos wait in Review (app, with a team gallery) |

This computer's answers to "show the repo name?" live in `~/.lolcommits/repo-names.tsv` (`repo`, tab, `show` or `hide`). With a team gallery the team's choice in `.meta/repos/` wins; your answers count only for repos the team hasn't decided yet, and are published to the team once.

Pictures land in `~/.lolcommits/<repo>/<timestamp>-<sha>.jpg`, along with an `index.tsv` (time, repo, branch, sha, subject, file).
Failures are logged to `~/.lolcommits/lolcommits.log`.

### Advanced share targets (command line only)

- **S3 or any other rclone remote:** `share.target rclone:<remote>:<bucket>/<path>`. Pick an EU region.
- **A server over ssh:** `share.target user@host:/srv/lolcommits` (rsync).

## Development

```bash
cd app
npm install
npm run dev          # run the app; LOLCOMMITS_TEST_PATTERN=1 uses test bars instead of the camera
cd src-tauri && cargo test
npm run build        # lolcommits.app + dmg in src-tauri/target/release/bundle
```

**Releases:** pushing a tag like `0.3.0` builds signed and notarized macOS and Linux packages ([release.yml](.github/workflows/release.yml)).
- **Signing:** with the Developer ID of entrecode GmbH (team `MAMAYY5H8H`, set in the workflow). Needs the secrets `APPLE_CERTIFICATE` (the .p12 as base64), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID` and `APPLE_PASSWORD` (app-specific password).
- **Signed build on your Mac:** `APPLE_SIGNING_IDENTITY="Developer ID Application: entrecode GmbH (MAMAYY5H8H)" npm run build`, the certificate must be in your keychain.
- **Without them:** the build is unsigned.
- **Google client:** the secrets `LOLCOMMITS_GDRIVE_CLIENT_ID` and `LOLCOMMITS_GDRIVE_CLIENT_SECRET` (entrecode's "Desktop app" client, Internal) are built into the app. For a local build or test, set the same environment variables.
- **Updates:** CI also builds signed update archives and `latest.json`, which the app reads from the latest release. They need the secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The public key is in `tauri.conf.json`. Drafts don't count as latest, so an update reaches users once the release is published.
- **Test an update locally:** `LOLCOMMITS_UPDATE_URL=http://…/latest.json` points the app at another manifest and checks after 3 seconds (http only with `dangerousInsecureTransportProtocol` in a test build).
- **Version:** keep `version` in `app/src-tauri/tauri.conf.json` in line with the tag.
