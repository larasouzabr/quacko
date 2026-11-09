#!/bin/bash
# Depois do `tauri build` no macOS: tira do registro de apps do sistema (LaunchServices)
# a cópia de build do Miau.app e o disco temporário que o empacotador do .dmg montou.
# Sem isso, aparece um segundo "Miau" na lista de apps. Os arquivos continuam onde estão.
LSR=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
[ -x "$LSR" ] || exit 0
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$LSR" -dump 2>/dev/null |
  sed -n 's/^path: *\(.*Miau\.app\) (0x[0-9a-f]*)$/\1/p' |
  sort -u |
  while IFS= read -r app; do
    case "$app" in
      "$ROOT"/* | /Volumes/dmg.*)
        "$LSR" -u "$app" 2>/dev/null && echo "desregistrado: $app"
        ;;
    esac
  done
for dmg in "$ROOT"/src-tauri/target.noindex/release/bundle/dmg/*.dmg \
  "$ROOT"/src-tauri/target.noindex/*-apple-darwin/release/bundle/dmg/*.dmg; do
  [ -f "$dmg" ] && echo "instalador: $dmg"
done
