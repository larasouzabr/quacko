# Contributing to Quacko

Thanks for wanting to help the duck! Bug reports, ideas and pull requests are all welcome.

## Before you start

- For a bug, open an issue with the **Bug report** template: what happened, what you expected, your operating system and Quacko version.
- For a new feature, open an issue first so we can agree on the idea before you spend time on code.
- Security problems go through the private form described in [SECURITY.md](SECURITY.md), never in a public issue.
- Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Setting up

You need [Rust](https://rustup.rs) (stable) and Node.js 20+. On macOS, also the Xcode command line tools; on Windows, the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev        # the app in development mode
scripts/demo.sh          # fires 4 sample notifications (macOS)
```

The [README](README.md#development) has the build commands for each system.

## Before opening a pull request

Run every test suite; CI runs the same ones on macOS and Windows:

```bash
npm test
npm run typecheck
(cd src-tauri && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings)
(cd src-tauri && cargo fmt --all --check)
bash scripts/tests/notify.test.sh
(cd src-tauri && cargo build -p quacko-hook) && QUACKO_NOTIFY=src-tauri/target.noindex/debug/quacko-hook bash scripts/tests/notify.test.sh
bash scripts/tests/prwatch.test.sh
bash scripts/tests/shell.test.sh
```

A few house rules:

- **Keep `notify.sh` and `quacko-hook` in step.** They're two implementations of the same hook (macOS and Windows); a behavior change goes into both, with a case in `scripts/tests/notify.test.sh`.
- **No code comments.** The codebase has none; prefer clear names and small functions, and explain the *why* in the pull request.
- **Small, focused pull requests**, one change each, with tests for what you changed.
- **Don't touch files outside the project** in tests: use temporary folders, as the existing tests do.
- **Pixel art** stays pixel art: integer scaling only, and new characters or accessories follow the existing `manifest.json` / `accessory.json` formats.

## License

By contributing, you agree that your code is released under the [MIT License](LICENSE). Character art is released under CC0-1.0.
