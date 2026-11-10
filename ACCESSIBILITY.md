# Accessibility

Quacko should let you know when work is done no matter how you use your computer. This page says what already works, where it falls short today, and how to tell us about a barrier.

## What Quacko does today

- **State is never shown by color alone.** Every card has an icon and a word (Done, Failed, Attention, Notice) next to the color, and the duck's pose changes too.
- **Reduced motion is respected.** With "Reduce motion" turned on in your system settings, the duck stops animating and cards appear without sliding.
- **Nothing depends on sound.** Every notification is visual; the sound only accompanies it, and **Play sound** in the menu turns it off. **Pause music** can be turned off too, and Do Not Disturb silences cards, sound and music pausing together.
- **Cards wait for you.** Hovering a card pauses its timer, and a finished CI run stays on screen until you close it.
- **Screens are labeled for assistive technology.** The close button has an accessible name, CI progress is exposed as a progress bar (for example, "7/12"), decorative pixel art is hidden from screen readers, and the color choices in Customize are announced by name.
- **Bigger character.** The duck can be drawn at 3, 4, 5, 6 or 8 screen pixels per art pixel.
- **Languages.** The interface is available in English and Brazilian Portuguese, following your system language by default.

## Supported environments

- macOS 11 or newer.
- Windows 10 and 11 (x64). Windows support is new and hasn't been tested with Narrator yet.

## Known limitations

- **The cards can't be reached with the keyboard.** The notification window is deliberately non-focusable so it never steals focus from what you're typing; that also means you can't Tab to a card. The same notifications are available in the tool that triggered them (Claude Code, Cursor or the PR on GitHub).
- **Screen readers don't announce cards yet.** Because the window never takes focus, VoiceOver and Narrator don't read new cards aloud.
- **Cards that aren't sticky close on their own** after about 8 seconds (between 2 and 30 for scripts that set it), which may be too short for some people.
- **Card text doesn't follow the system text size.** Only the character can be enlarged today.

## Reporting a barrier

If something in Quacko gets in your way, please [open an issue](https://github.com/larasouzabr/quacko/issues/new?template=bug_report.yml) and mention "accessibility" in the title. Tell us what you were trying to do, which assistive technology or setting you use, and your operating system. Barriers are treated as bugs, not feature requests.
