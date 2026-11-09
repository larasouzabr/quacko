#!/bin/bash
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${QUACKO_HOOKS_DIR:-$HOME/.claude/hooks}"
STAMP="$(date +%Y%m%d-%H%M%S)"

install_one() {
  local src="$HERE/$1" dst="$DEST/$2"
  if [ -f "$dst" ] && ! cmp -s "$src" "$dst"; then
    local bak="$dst.bak"
    [ -e "$bak" ] && bak="$dst.bak-$STAMP"
    cp -p "$dst" "$bak"
    echo "  backup: $bak"
  fi
  cp "$src" "$dst"
  chmod 755 "$dst"
  echo "  instalado: $dst"
}

mkdir -p "$DEST"
echo "Quacko: instalando scripts em $DEST"
install_one notify.sh notify.sh
install_one prwatch prwatch

N="$DEST/notify.sh"
cat <<EOF

1) Claude Code (~/.claude/settings.json)
   Hooks antigos que chamam "notify.sh <som>" já mostram o pato sem mudar nada.
   Para o cartão dizer qual pedido o Claude terminou, use estes hooks
   (o UserPromptSubmit guarda o pedido; ele não toca som nem mostra nada):

  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "$N --from claude" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "$N --from claude --sound /System/Library/Sounds/Glass.aiff" }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "$N --from claude --sound /System/Library/Sounds/Ping.aiff" }] }
    ]
  }

2) Cursor (~/.cursor/hooks.json), se você usa:

  {
    "version": 1,
    "hooks": {
      "beforeSubmitPrompt": [{ "command": "$N --from cursor" }],
      "stop": [{ "command": "$N --from cursor --sound /System/Library/Sounds/Glass.aiff" }]
    }
  }

3) prwatch: espera a CI de um PR e avisa. Para chamar só "prwatch", coloque-o no PATH:
EOF

if [ -L "$HOME/.zshrc" ] && readlink "$HOME/.zshrc" | grep -q '^/nix/store'; then
  echo "   Seu ~/.zshrc é gerenciado pelo Nix (home-manager): adicione na sua config do home-manager"
  echo "     programs.zsh.shellAliases.prwatch = \"$DEST/prwatch\";"
  echo "   ou crie um link numa pasta que já esteja no PATH:"
else
  echo "   adicione ao ~/.zshrc:  alias prwatch=\"$DEST/prwatch\""
  echo "   ou crie um link numa pasta que já esteja no PATH:"
fi
echo "     mkdir -p ~/.local/bin && ln -sf \"$DEST/prwatch\" ~/.local/bin/prwatch"
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) echo "   (atenção: ~/.local/bin não está no seu PATH agora)" ;;
esac
cat <<EOF

   Uso: prwatch 1234   |   prwatch https://github.com/dono/repo/pull/1234   |   prwatch (PR da branch atual)

Teste rápido (com o app Quacko aberto):
  $N --source ci --status fail --title "PR #1" --message "Teste do Quacko" --project teste
EOF
