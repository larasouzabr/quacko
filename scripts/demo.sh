#!/bin/bash
# Dispara 4 avisos de exemplo (Claude e CI), um de cada estado do gato.
# Substitui o item "Testar notificação" do tray até ele existir (Marco 4).
#
# Uso: scripts/demo.sh            (porta 4747 ou $QUACKO_PORT)
#      scripts/demo.sh progresso  (barra de progresso de um PR)
#      INTERVALO=4 scripts/demo.sh (segundos entre um estado e outro)
set -u
PORT="${QUACKO_PORT:-4747}"
INTERVALO="${INTERVALO:-2.5}"

post() {
  curl -s -o /dev/null -w '%{http_code}' --max-time 2 -X POST "http://127.0.0.1:$PORT/notify" \
    -H 'Content-Type: application/json' -d "$1"
}

if ! curl -s --max-time 1 "http://127.0.0.1:$PORT/health" >/dev/null; then
  echo "O Quacko não está respondendo na porta $PORT. Abra o app (npm run tauri dev) e tente de novo." >&2
  exit 1
fi

# "scripts/demo.sh progresso": um PR com 12 checks; o mesmo cartão vai se atualizando.
if [ "${1:-}" = progresso ]; then
  for done in 0 3 7 10 12; do
    failed=0
    [ "$done" -ge 7 ] && failed=1
    if [ "$done" = 12 ]; then status=fail; msg="1 de 12 checks falhou"; ttl=9000
    elif [ "$failed" = 1 ]; then status=fail; msg="$done de 12 checks concluídos · 1 falhou"; ttl=6000
    else status=info; msg="$done de 12 checks concluídos"; ttl=6000; fi
    body=$(printf '{"source":"ci","status":"%s","title":"PR #445 · Corrige o parser de datas","message":"%s","project":"pondworks/breadcrumbs#445","ttl_ms":%s,"progress":{"done":%s,"total":12,"failed":%s}}' "$status" "$msg" "$ttl" "$done" "$failed")
    echo "$done/12 -> HTTP $(post "$body")"
    [ "$done" = 12 ] || sleep "$INTERVALO"
  done
  exit 0
fi

# Conteúdo realista: é o que os scripts do Marco 5 vão mandar.
# ttl de 7 s com intervalo de 2,5 s: no máximo 3 na tela, sem empurrar ninguém.
PAYLOADS=(
  'ok|{"source":"claude","status":"ok","title":"Claude Code","message":"Respondeu: “adicione testes para o parser de datas”","project":"lagoa-api","ttl_ms":7000}'
  'fail|{"source":"ci","status":"fail","title":"PR #1234: checks falharam","message":"Corrige fuso horário no parser de datas","project":"pondworks/lagoa-api","url":"https://github.com","ttl_ms":7000}'
  'attention|{"source":"claude","status":"attention","title":"Claude Code","message":"Claude precisa da sua permissão para usar Bash","project":"quacko-watch","ttl_ms":7000}'
  'info|{"source":"custom","status":"info","title":"Quacko","message":"Aviso de um script seu (quacko run, cron…)","project":"meu-script","ttl_ms":7000}'
)
for item in "${PAYLOADS[@]}"; do
  name="${item%%|*}"
  body="${item#*|}"
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 -X POST "http://127.0.0.1:$PORT/notify" \
    -H 'Content-Type: application/json' -d "$body")
  echo "$name -> HTTP $code"
  [ "$name" = info ] || sleep "$INTERVALO"
done
