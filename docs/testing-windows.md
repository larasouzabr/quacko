# Testing Quacko on Windows

Quacko's Windows build is compiled and unit-tested in CI, but it hasn't been tried on a real machine yet. This checklist takes about 15 minutes. Please [open an issue](https://github.com/larasouzabr/quacko/issues/new?template=bug_report.yml) with what failed, your Windows version (Settings → System → About) and, if you can, a screenshot.

## Install

- [ ] Download **Quacko_x64-setup.exe** from the [latest release](https://github.com/larasouzabr/quacko/releases/latest) and run it. If SmartScreen warns, click **More info → Run anyway**.
- [ ] The duck shows up in the system tray (it may be hidden behind the **^** arrow next to the clock).
- [ ] The welcome screen opens on its own.

## Cards

- [ ] Tray menu → **Notifications → Test notification**: four cards appear at the top right, one after the other, each with the duck, a sound and the right colour.
- [ ] While a card is on screen, keep typing in another app: the card never takes the keyboard focus.
- [ ] Hover a card: it stops closing and the whole message shows. Click the **×**: it closes.
- [ ] **Preferences → Text size → Larger**, then test again: the text and the card are bigger.
- [ ] **Recent** lists the test cards; clicking one shows it again.

## Claude Code and Cursor

- [ ] **Connect → Claude Code**: the card says it's connected. Open `%USERPROFILE%\.claude\settings.json` and check the hooks point to `quacko-hook.exe`.
- [ ] Ask Claude Code something short. When it answers, a card shows what you asked, with the project name.
- [ ] Play music in Spotify or a YouTube video in the browser, ask Claude again: the music pauses when the card appears.
- [ ] **Notifications → Pause music** unchecked: the music keeps playing. **Play sound** unchecked: no sound.
- [ ] **Do Not Disturb** on: no card, no sound, music keeps playing. The notification still appears in **Recent**, marked as silenced.
- [ ] If you use Cursor: **Connect → Cursor**, then ask the agent something.

## Terminal (PowerShell)

- [ ] **Connect → Terminal…**: the card says a line was copied. Run `notepad $PROFILE`, paste the line at the end, save and open a new PowerShell window.
- [ ] Run `Start-Sleep 35`: a card shows "Start-Sleep 35" and "35s".
- [ ] Run `pwsh -c "Start-Sleep 35; exit 3"`: a red card with "exit 3" stays until you close it.
- [ ] Press Ctrl+C on a long command: no card.

## GitHub

- [ ] **Connect → GitHub (PRs and workflows)** with the GitHub CLI logged in (`gh auth login`), or **Sign in** on the welcome screen.
- [ ] Open a PR or push to a repository with Actions: when CI finishes, a card shows the result. Clicking it opens the PR or the run in the browser.

## Uninstall

- [ ] **Settings → Apps → Quacko → Uninstall** removes the app. The hooks stay in `settings.json` until you disconnect them from the menu first.
