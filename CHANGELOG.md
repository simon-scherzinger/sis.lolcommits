# Changelog

## Unreleased

- **Fits small windows:** the top bar stays on one line down to the smallest window size, long repo and branch names are shortened with "…" instead of breaking the date over three lines, and in Review the swipe buttons stay on screen even with a banner showing. The photo viewer keeps its info bar visible for long commit messages.
- **Loading placeholders:** pictures, the team gallery and the share preview shimmer while they load, instead of empty grey boxes or a flash of "nothing here yet".
- **Share only what you have seen:** the Share button waits until the preview is on screen. Before, a quick click could share before the preview had loaded.
- **Icons instead of emojis:** buttons, the setup assistant and Review use simple line icons.
- **Filters stay in reach:** in My shots and Team the search and filter bar sticks under the top bar while you scroll. In the setup assistant and in dialogs, the Back/Next and Cancel/Close buttons stay visible on small windows.
- **Warm-up slider** in the app's style instead of the blue system slider.
- **Compact toolbars on narrow windows:** below 900 px the toolbar buttons show only their icon, with the label as a tooltip, so the bar stays on one line.
- **Team gallery in order:** pictures from the same day are sorted with the most recently shared first. Before, they were in effect sorted by repo folder and a random part of the file name. Still no time of day is stored or shown.
- **Renew old Google sign-ins:** if the team gallery still goes through a sign-in made with rclone's shared Google client (older versions did that), a banner asks you to sign in again. Google turns that client off in 2026, and sharing would stop working then. "Sign in again" opens the team setup, where "Sign in with Google" replaces the old sign-in.
- **Faster window:** settings are read once and again only when they change, instead of starting `git` a dozen times per refresh. A refresh after each photo, toggle or share takes about 20 ms instead of about 320 ms.
- **Team gallery opens at once:** it shows the pictures already on this computer right away and syncs with Google Drive in the background (the Refresh button spins meanwhile). Opening the tab again within a minute doesn't sync again; Refresh always does.
- **Review intro tells the truth:** it no longer says the repo name is always blurred. Repos you show by name aren't blurred.
- **Team gallery problem banner** has a "Change team gallery…" button, so you can switch to a target the app can use (an ssh target set up from the command line only works there).
- **Review oldest first:** the Review stack starts with the oldest photo, so you go through them in the order they were taken. My shots still shows the newest first.
- **Only pictures in the repo folders:** the small `.tsv` file with date, name and commit message that goes with each shared picture now sits in a hidden `.meta/` folder of the team gallery instead of next to the picture. The app moves the ones of your own pictures there on its next start. Pictures shared by older versions still show, and deleting removes the file in both places. Teammates need this version (app or command line tool) to see pictures shared with it.
- **Reminder to review:** once a day, from 10:00, a notification says how many new photos are waiting in Review. Clicking it opens Review. Only with a team gallery set up, not while paused and not while the window is open. Turn it off in Settings › Team gallery › "Remind me to review". The first reminder asks macOS for permission to show notifications.
- **Settings › Repos:** one row per repo with three switches. **Photos** off: no photos in this repo, in all its clones and worktrees (`lolcommits.skipRepo`). **Share** off: its photos stay on this computer and never show in Review (`lolcommits.neverShare`). **Name**: shown or hidden in the team gallery, as before. The command line tool and the commit hook follow the same settings.
- **Team settings in the shared drive:** whether a repo's name is shown is now decided once for the whole team: the first person who shares from a repo gets the question, everyone else uses that answer ("set by the team" in Settings › Repos), and changing it changes it for everyone. Hidden repos are stored only as a hash, so their names don't end up in the drive. On the first start with this version your earlier answers are published for repos the team hasn't decided yet; where they differ, "hide" wins. The team can also set a limit for how long shared pictures stay (Settings › Team gallery); nobody keeps them longer, anyone may remove theirs earlier. The command line tool follows both.
- **Only deleting is red:** confirmations like "Show name" or "Share every photo automatically" use the normal button.

## 0.5.3

- **Camera free again after a photo:** lolcommits now lets go of the camera after every photo. Before, Photo Booth, FaceTime or WhatsApp got no picture until you quit lolcommits.

## 0.5.2

- **Repo name from git:** pictures are filed under the name of the `origin` remote (e.g. `hec.dsb-layer`), not the folder name, so every git worktree or clone in another folder counts as the same repo. Without a remote it's the main worktree's folder. Pictures taken before stay where they are.
- **Commit message across the picture:** at most two lines that use the full width; longer messages end with "…". Before, it was up to four narrow lines in the middle.

## 0.5.1

- **Update menu shows the right version:** the app checks for updates every hour instead of every six, and while updating it shows the version it is really installing. Before, the menu could still offer an older version (e.g. "Update to 0.4.2") although the newest one got installed.
- **"Check for updates…" opens the Updates section** in Settings instead of the top of the page.

## 0.5.0

Feedback from the first teammates.

- **Only your work repos:** Settings › Capture › "Only in these folders" takes photos only in repos inside the folders you pick, e.g. your work folder but not your experiments.
- **Select several pictures:** "Select" in My shots, or ⌘-click / ⇧-click, then delete or unshare them together, with progress.
- **Right-click a picture:** open, share or unshare, copy the commit message, show in Finder, delete. The browser menu ("Look Up", "Download Image") is gone.
- **You see what's happening:** every button shows a spinner while it works and can't be clicked twice. Deleting keeps its dialog open until it's done. Taking a picture back from Google Drive is about twice as fast.
- **Google sign-in, faster and without asking twice:** lolcommits now signs in with its own Google client instead of rclone's shared one, which Google throttled and retired in 2026. Teams outside entrecode enter their company's client once in the setup ([how](README.md#google-sign-in-without-drive-for-desktop)); Google Drive for desktop still needs nothing.
  - After the sign-in the browser shows a lolcommits page instead of rclone's plain text.
  - Switching to another shared drive no longer asks Google again, and an existing sign-in is reused.
  - Cancel any time; it gives up after 5 minutes.
  - The setup says that pictures go to a folder "lolcommits" inside the shared drive, and the label shows it (e.g. "Google Drive › Team › lolcommits").
- **Review doesn't wait for uploads:** swiping right uploads in the background, the next card is there at once. Picture and sidecar go up in one call.
- **Calmer look:** cards use the full width, no drop shadows, all controls in a toolbar are the same height, the year for Wrapped is only asked when there is more than one.
- **Dates in English:** "25 Nov 2026, 21:55" and "Thu 06:00" instead of the system language mixed into the English interface.
- **Paused:** shown once (banner) instead of banner plus badge; the menu bar icon is dimmed while paused.
- **A normal window:** while the window is open, lolcommits is in the Dock and in ⌘-Tab. ⌘Q closes the window and photos keep coming; quit from the menu bar menu.
- **Privacy guide** and every tab open at the top.
- **Apple Silicon only:** the Mac download no longer includes Intel code, so it's smaller and builds faster.

## 0.4.2

- **Team gallery setup for everyone:** setting up a shared drive no longer fails with "can't write there … File not found" if you aren't a manager of the shared drive. Content managers are enough; if your role is too low, the app says so.
- **Taking pictures back:** if your role doesn't allow deleting for good, pictures go to the shared drive's trash instead of silently staying in the gallery.

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
