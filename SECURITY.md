# Security policy

## Supported versions

Only the latest release gets security fixes. If you build from source, use the latest commit on `main`.

## Reporting a vulnerability

Please don't open a public issue for security problems. Report them privately through
[this repository's private reporting form](https://github.com/larasouzabr/quacko/security/advisories/new)
(**Security → Report a vulnerability**). Only the maintainer can see the report.

Include what you found, how to reproduce it, and which version and operating system you used. You should get an answer within a week. Once a fix is out, the advisory is published with credit to you, unless you'd rather stay anonymous.

## What's in scope

Quacko runs entirely on your computer, so the most sensitive parts are:

- the local HTTP server on `127.0.0.1:4747` (it should reject web pages, other hosts, oversized bodies and floods);
- the files the app edits when you connect an integration: `~/.claude/settings.json`, `~/.cursor/hooks.json` and the hook scripts it installs;
- the GitHub token, stored in the macOS Keychain or the Windows Credential Manager;
- importing accessories (zip files with images).

Problems in Claude Code, Cursor, the GitHub CLI or `nowplaying-cli` themselves should go to those projects.
