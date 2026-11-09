#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLT=/Library/Developer/CommandLineTools
[ -d "$CLT" ] || { echo "Instale as ferramentas da Apple: xcode-select --install" >&2; exit 1; }
SDK="$(env -i DEVELOPER_DIR="$CLT" /usr/bin/xcrun --sdk macosx --show-sdk-path)"
NODE_DIR="$(dirname "$(command -v node)")"
CARGO_DIR="$(dirname "$(command -v cargo || echo "$HOME/.cargo/bin/cargo")")"

exec env -i HOME="$HOME" USER="${USER:-$(id -un)}" LANG="${LANG:-en_US.UTF-8}" TERM="${TERM:-dumb}" \
  PATH="/usr/bin:/bin:/usr/sbin:/sbin:$CLT/usr/bin:$CARGO_DIR:$NODE_DIR" \
  DEVELOPER_DIR="$CLT" SDKROOT="$SDK" \
  bash -c 'cd "$1" && npx tauri build --target universal-apple-darwin && bash scripts/after-build.sh' _ "$ROOT"
