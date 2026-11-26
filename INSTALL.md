# Installing lolcommits with a coding agent

These steps are written for Claude Code or another coding agent installing lolcommits on the user's machine.
People can follow them too.

To start, tell Claude Code:

> Install lolcommits following https://github.com/simon-scherzinger/sis.lolcommits/blob/main/INSTALL.md

---

## Instructions for the agent

Do the steps in order. Ask the user before anything marked **(ask)**. Stop and report if a step fails.

The goal: the user has lolcommits.app and works through its setup assistant themselves.
Don't change the camera, git hook or sharing settings for them. The assistant asks the user about these on purpose (camera permission, privacy note, team gallery).

### 1. Detect the platform

```bash
uname -s          # Darwin = macOS, Linux = Linux
git --version     # 2.54+ is needed for "every repo"
```

### 2. Get the app (ask)

Tell the user what you will download and from where, then:

**macOS:**

```bash
gh release download --repo simon-scherzinger/sis.lolcommits --pattern '*.dmg' --dir "$(mktemp -d)"
```

1. Mount the dmg with `hdiutil attach`, copy `lolcommits.app` to `/Applications`, then detach.
2. Unsigned builds (no Apple signing in the release yet): tell the user that macOS will refuse the first start. They allow it under System Settings › Privacy & Security › "Open Anyway". Don't remove the quarantine flag yourself.

**Linux:** download the `.deb` (Debian/Ubuntu: `sudo apt install ./lolcommits_*.deb`, leave the password to the user) or the `.AppImage` (`chmod +x`, put it in `~/.local/bin/lolcommits-app`).

**No release yet:** build from source. This needs Rust and Node. Ask before installing Rust with rustup.

```bash
git clone https://github.com/simon-scherzinger/sis.lolcommits.git ~/Developer/sis.lolcommits
cd ~/Developer/sis.lolcommits/app && npm install && npm run build
# macOS: src-tauri/target/release/bundle/macos/lolcommits.app → /Applications
```

### 3. Start it

```bash
open -a lolcommits        # macOS
lolcommits-app &          # Linux
```

Tell the user to follow the setup assistant: privacy note, camera, "every commit", and the optional team gallery.
Don't click through it for them.

### 4. Report back

Tell the user:
- lolcommits lives in the menu bar. The icon flashes when it takes a photo, and its menu has pause, test photo and share.
- For the team gallery, a Google shared drive (e.g. "lolcommits") with the team as members is enough. With Google Drive for desktop, no sign-in is needed. Without it, "Sign in with Google" needs a Google client of the company (built in for entrecode; others see the README section "Google sign-in without Drive for desktop").
- In a company: read [PRIVACY.md](PRIVACY.md) before sharing.

## Command line only (no app)

For servers, Linux without a desktop, or people who prefer the terminal.

1. **Dependencies (ask):** macOS `brew install ffmpeg imagemagick`, Debian/Ubuntu `sudo apt-get install -y ffmpeg fonts-dejavu-core`.
2. **Code:**
   ```bash
   git clone https://github.com/simon-scherzinger/sis.lolcommits.git ~/Developer/sis.lolcommits
   mkdir -p ~/.local/bin && ln -sf ~/Developer/sis.lolcommits/lolcommits ~/.local/bin/lolcommits
   lolcommits doctor   # must end with "all good ✓"
   ```
3. **Test without camera:**
   ```bash
   LOLCOMMITS_NO_APP=1 LOLCOMMITS_NO_OPEN=1 LOLCOMMITS_DIR="$(mktemp -d)" \
     LOLCOMMITS_INPUT="-f lavfi -i testsrc2=size=1280x720:rate=30" lolcommits test
   ```
4. **Camera test:** the user runs `lolcommits test` themselves (macOS asks for camera access for their terminal).
5. **Enable (ask):** `lolcommits enable --global` (git 2.54+) or `lolcommits enable` inside a repo.

## Uninstall

1. In the app: Privacy › Delete all (optionally including shared pictures), then Settings › "Photo on every commit" off.
2. Quit it from the menu bar and delete `/Applications/lolcommits.app`.
3. Command line: `lolcommits forget --all`, `lolcommits disable --global`, `rm ~/.local/bin/lolcommits`.
