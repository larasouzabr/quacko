#!/bin/bash
# Dispara 4 avisos de exemplo (Claude e CI), um de cada estado do gato.
# Substitui o item "Testar notificação" do tray até ele existir (Marco 4).
#
# Uso: scripts/demo.sh            (porta 4747 ou $MIAU_PORT)
#      INTERVALO=4 scripts/demo.sh (segundos entre um estado e outro)
set -u
PORT="${MIAU_PORT:-4747}"
INTERVALO="${INTERVALO:-2.5}"

if ! curl -s --max-time 1 "http://127.0.0.1:$PORT/health" >/dev/null; then
  echo "O Miau não está respondendo na porta $PORT. Abra o app (npm run tauri dev) e tente de novo." >&2
  exit 1
fi

# Conteúdo realista: é o que os scripts do Marco 5 vão mandar.
# ttl de 7 s com intervalo de 2,5 s: no máximo 3 na tela, sem empurrar ninguém.
PAYLOADS=(
  'ok|{"source":"claude","status":"ok","title":"Claude Code","message":"Respondeu: “adicione testes para o parser de datas”","project":"lagoa-api","ttl_ms":7000}'
  'fail|{"source":"ci","status":"fail","title":"PR #1234: checks falharam","message":"Corrige fuso horário no parser de datas","project":"pondworks/lagoa-api","url":"https://github.com","ttl_ms":7000}'
  'attention|{"source":"claude","status":"attention","title":"Claude Code","message":"Claude precisa da sua permissão para usar Bash","project":"miau-watch","ttl_ms":7000}'
  'info|{"source":"custom","status":"info","title":"Miau","message":"Aviso de um script seu (miau run, cron…)","project":"meu-script","ttl_ms":7000}'
)
for item in "${PAYLOADS[@]}"; do
  name="${item%%|*}"
  body="${item#*|}"
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 -X POST "http://127.0.0.1:$PORT/notify" \
    -H 'Content-Type: application/json' -d "$body")
  echo "$name -> HTTP $code"
  [ "$name" = info ] || sleep "$INTERVALO"
done
