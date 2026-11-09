#!/bin/bash
# Gera o .dmg universal (Apple Silicon + Intel) com o toolchain da Apple (Xcode Command
# Line Tools), num ambiente limpo, como num Mac comum ou no GitHub Actions.
#
# Por que o ambiente limpo: dentro de um shell do Nix, o compilador C padrão só gera
# arm64, e a parte Intel falha (o aws-lc, usado no TLS, tem assembly x86). Fora do Nix
# este script dá o mesmo resultado que "tauri build --target universal-apple-darwin".
#
# Uso: npm run build:mac:universal   (ou bash scripts/build-universal.sh)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLT=/Library/Developer/CommandLineTools
[ -d "$CLT" ] || { echo "Instale as ferramentas da Apple: xcode-select --install" >&2; exit 1; }
# Ambiente limpo também aqui: um SDKROOT herdado (ex.: do Nix) faria o xcrun devolvê-lo.
SDK="$(env -i DEVELOPER_DIR="$CLT" /usr/bin/xcrun --sdk macosx --show-sdk-path)"
NODE_DIR="$(dirname "$(command -v node)")"
CARGO_DIR="$(dirname "$(command -v cargo || echo "$HOME/.cargo/bin/cargo")")"

exec env -i HOME="$HOME" USER="${USER:-$(id -un)}" LANG="${LANG:-en_US.UTF-8}" TERM="${TERM:-dumb}" \
  PATH="/usr/bin:/bin:/usr/sbin:/sbin:$CLT/usr/bin:$CARGO_DIR:$NODE_DIR" \
  DEVELOPER_DIR="$CLT" SDKROOT="$SDK" \
  bash -c 'cd "$1" && npx tauri build --target universal-apple-darwin && bash scripts/after-build.sh' _ "$ROOT"
