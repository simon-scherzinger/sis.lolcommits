# Changelog

## 0.4.1

- **Download without warning:** the `.dmg` itself is now notarized too, not only the app inside, so macOS opens a downloaded disk image without asking.
- **First update through the app:** if you have 0.4.0, the menu bar offers "Update to 0.4.1 and restart". Nothing else changes in the app.

## 0.4.0

First public release, as a menu bar app. (0.3.0 was tagged but never published.)

- **App instead of a script:** download, open, done. A setup assistant handles the camera, the git hook and the team gallery, without terminal or Homebrew.
- **Menu bar:** the icon flashes while a photo is taken. Pause for an hour, until tomorrow or until you resume, take a test photo, review new photos.
- **Commits stay fast:** the git hook only queues a job, the app takes the photo in the background. One camera permission for the app instead of one per terminal or IDE.
- **Hot or not:** new photos wait in a review stack. Swipe right to share, left to keep private, ⌘Z to undo.
- **Team gallery on Google Drive:** pick a shared drive from Google Drive for desktop or sign in with Google. One folder per repo, opt-in per repo; other repos go to `other-repos` with the repo label and commit hash blurred.
- **Privacy:** sharing is opt-in, every picture is reviewed as uploaded, no metadata, no times of day, unshare everything in one click. Shared pictures don't expire unless you set a period. See [PRIVACY.md](PRIVACY.md) before rolling it out in a company.
- **Extras:** gallery with search, timelapse video (needs ffmpeg), year-in-review contact sheet.
- **Updates in the app:** lolcommits looks for new releases every few hours and shows "Update to X and restart" in the menu bar. One click downloads, installs and restarts. Settings › Updates shows the version, what's new and a switch to turn the check off.
- **Safe updates:** update files are signed with a separate key, and the app installs nothing with a wrong signature. It waits for a photo in progress before restarting. The camera permission stays.
- **Signed and notarized** for macOS with the Developer ID of entrecode GmbH.
- **Command line:** the bundled `lolcommits` script still works on its own on macOS and Linux.
