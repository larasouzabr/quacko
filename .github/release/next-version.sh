#!/bin/bash
# Próxima versão a partir da última tag e dos labels da PR.
# Uso: next-version.sh <última tag ou vazio> "<labels separados por espaço>"
# Sem label: patch. Label "minor": minor. Label "major": major.

set -euo pipefail

latest="${1:-v0.0.0}"
labels=" ${2:-} "

case "$latest" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "tag inválida: $latest" >&2; exit 1 ;;
esac

IFS=. read -r major minor patch <<<"${latest#v}"
case "$labels" in
  *" major "*) major=$((major + 1)); minor=0; patch=0 ;;
  *" minor "*) minor=$((minor + 1)); patch=0 ;;
  *) patch=$((patch + 1)) ;;
esac

echo "$major.$minor.$patch"
