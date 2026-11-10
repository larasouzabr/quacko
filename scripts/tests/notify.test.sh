#!/bin/bash

HERE="$(cd "$(dirname "$0")" && pwd)"
NOTIFY="${QUACKO_NOTIFY:-$HERE/../notify.sh}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export QUACKO_DRY_RUN=1 QUACKO_STATE_DIR="$WORK/state" QUACKO_LOG="$WORK/log" QUACKO_CONFIG="$WORK/config.json"

pass=0
fail=0
ok() { pass=$((pass + 1)); }
bad() {
  fail=$((fail + 1))
  echo "FALHOU: $1"
  [ -n "${2:-}" ] && echo "   $2"
}

run() {
  local in="$1"
  shift
  if [ -n "$in" ]; then
    printf '%s' "$in" | "$NOTIFY" "$@" >"$WORK/out" 2>"$WORK/err"
  else
    "$NOTIFY" "$@" </dev/null >"$WORK/out" 2>"$WORK/err"
  fi
  RC=$?
  OUT=$(cat "$WORK/out")
  PAYLOAD=$(sed -n 's/^payload: //p' "$WORK/err")
  EFFECTS=$(sed -n 's/^effects: //p' "$WORK/err")
}

expect_effects() {
  local pause="$2" sound="$3" got_pause got_sound
  got_pause=$(printf '%s' "$EFFECTS" | sed -n 's/^pause=\([01]\) .*/\1/p')
  got_sound=$(printf '%s' "$EFFECTS" | sed -n 's/^pause=[01] sound=//p')
  if [ "$got_pause" != "$pause" ]; then
    bad "$1" "pausa: '$EFFECTS'"
  elif [ "$sound" = yes ] && [ -z "$got_sound" ]; then
    bad "$1" "sem som: '$EFFECTS'"
  elif [ "$sound" = no ] && [ -n "$got_sound" ]; then
    bad "$1" "som inesperado: '$EFFECTS'"
  else
    ok
  fi
}

expect_payload() {
  if [ -z "$PAYLOAD" ]; then
    bad "$1" "nenhum aviso gerado"
  elif printf '%s' "$PAYLOAD" | jq -e "$2" >/dev/null 2>&1; then
    ok
  else
    bad "$1" "$PAYLOAD"
  fi
}
expect_no_payload() { [ -z "$PAYLOAD" ] && ok || bad "$1" "aviso inesperado: $PAYLOAD"; }
expect_stdout() { [ "$OUT" = "$2" ] && ok || bad "$1" "stdout='$OUT' esperado='$2'"; }
expect_rc0() { [ "$RC" = 0 ] && ok || bad "$1" "saiu com $RC"; }

suite() {
rm -rf "$WORK/state"
run "" --source ci --status fail --message "PR #1: checks falharam" --project teste
expect_payload "flags explícitas" '.source=="ci" and .status=="fail" and .message=="PR #1: checks falharam" and .project=="teste"'
expect_stdout "flags explícitas: stdout vazio" ""

run '{"cwd":"/tmp/meu-projeto"}' --source claude --status ok
expect_payload "cwd vira project" '.status=="ok" and .project=="meu-projeto"'

S='"session_id":"abc-123","cwd":"/Users/x/lagoa-api"'
run "{$S,\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"adicione testes\\npara o parser   de datas\"}"
expect_no_payload "UserPromptSubmit só guarda o pedido"
expect_stdout "UserPromptSubmit não escreve no stdout (iria para o contexto do Claude)" ""
expect_rc0 "UserPromptSubmit sai com 0"

run "{$S,\"hook_event_name\":\"Stop\",\"last_assistant_message\":\"Pronto.\"}" --sound /System/Library/Sounds/Glass.aiff
expect_payload "Stop mostra o pedido respondido" '.source=="agent" and .title=="Claude Code" and .status=="ok" and .project=="lagoa-api" and .message=="“adicione testes para o parser de datas”"'
expect_stdout "Stop: stdout vazio" ""
[ -z "$(ls "$WORK/state" 2>/dev/null | grep prompt-)" ] && ok || bad "pedido apagado depois de usado"

run "{$S,\"hook_event_name\":\"Stop\"}"
expect_payload "Stop sem pedido guardado: sem mensagem (texto padrão do app)" '.status=="ok" and (has("message") | not)'

run "{$S,\"hook_event_name\":\"Stop\"}" /System/Library/Sounds/Glass.aiff
expect_payload "forma antiga + hook Stop" '.status=="ok" and .title=="Claude Code"'
run "" /System/Library/Sounds/Glass.aiff
expect_no_payload "forma antiga sem stdin: só som e pausa"

run "{$S,\"hook_event_name\":\"Notification\",\"notification_type\":\"permission_prompt\",\"message\":\"Claude needs your permission to use Bash\"}"
expect_payload "Notification de permissão = attention" '.status=="attention" and .message=="Claude needs your permission to use Bash"'
run "{$S,\"hook_event_name\":\"Notification\",\"notification_type\":\"auth_success\",\"message\":\"ok\"}"
expect_payload "Notification auth_success = info" '.status=="info"'
run "{$S,\"hook_event_name\":\"Notification\",\"message\":\"x\"}" --status fail
expect_payload "--status explícito vence o adaptador" '.status=="fail"'

run "{$S,\"hook_event_name\":\"StopFailure\"}"
expect_payload "StopFailure = fail" '.status=="fail"'

run '{"session_id":"s1","hook_event_name":"UserPromptSubmit","prompt":"pedido um"}'
run '{"session_id":"s2","hook_event_name":"UserPromptSubmit","prompt":"pedido dois"}'
run '{"session_id":"s1","hook_event_name":"Stop"}'
expect_payload "pedido certo por sessão" '.message=="“pedido um”"'

LONG=$(printf 'a%.0s' $(seq 1 300))
run "{\"session_id\":\"s3\",\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"$LONG\"}"
run '{"session_id":"s3","hook_event_name":"Stop"}'
expect_payload "pedido longo cortado em 100 caracteres" '(.message | length) == 102 and (.message | endswith("…”"))'
run '{"session_id":"s4","hook_event_name":"UserPromptSubmit","prompt":"diga \"oi\" $(rm -rf /) `x` \\\\ 🐱"}'
run '{"session_id":"s4","hook_event_name":"Stop"}'
expect_payload "caracteres especiais preservados como texto" '.message=="“diga \"oi\" $(rm -rf /) `x` \\\\ 🐱”"'

run '{"session_id":"../../etc/x","hook_event_name":"UserPromptSubmit","prompt":"p"}'
[ ! -e "$WORK/etc" ] && ls "$WORK/state" | grep -q '^prompt-claude-etcx$' && ok || bad "session_id higienizado"

C='"conversation_id":"conv-9","workspace_roots":["/Users/x/meu-app"]'
run "{$C,\"hook_event_name\":\"beforeSubmitPrompt\",\"prompt\":\"refatore o login\"}"
expect_no_payload "beforeSubmitPrompt só guarda o pedido"
expect_stdout "beforeSubmitPrompt libera o envio" '{"continue":true}'

run "{$C,\"hook_event_name\":\"stop\",\"status\":\"completed\",\"loop_count\":0}"
expect_payload "Cursor completed = ok com pedido" '.source=="agent" and .title=="Cursor" and .status=="ok" and .project=="meu-app" and .message=="“refatore o login”"'
expect_stdout "Cursor stop responde {}" '{}'

run "{$C,\"hook_event_name\":\"stop\",\"status\":\"error\"}" --from cursor
expect_payload "Cursor error = fail" '.status=="fail"'

run "{$C,\"hook_event_name\":\"stop\",\"status\":\"aborted\"}"
expect_no_payload "Cursor aborted não avisa"
expect_stdout "Cursor aborted ainda responde {}" '{}'

run 'isso não é json' --from claude
expect_rc0 "stdin inválido sai com 0"
expect_no_payload "stdin inválido sem status: nada"
run "" --status talvez --source hacker
expect_no_payload "status/source fora da lista são ignorados"
run "" --status info --message
expect_payload "flag sem valor no fim não trava" '.status=="info" and .source=="custom"'
run "" --desconhecida --status ok
expect_payload "opção desconhecida é ignorada" '.status=="ok"'

run "" --source ci --status fail --sticky --title "PR #1"
expect_payload "--sticky marca o cartão" '.sticky == true and .title == "PR #1"'
run "" --source ci --status ok
expect_payload "sem --sticky não manda o campo" '(has("sticky") | not)'

run "" --status info --message "$(printf 'tab\there\nnova linha "aspas" \\ barra')" --title 'título com "aspas"'
expect_payload "texto com controle e aspas vira JSON válido" '.message == "tab\there\nnova linha \"aspas\" \\ barra" and .title == "título com \"aspas\""'
run '{"session_id":"s5","hook_event_name":"UserPromptSubmit","prompt":"ação\tcom   ünïcode\nem várias linhas"}'
run '{"session_id":"s5","hook_event_name":"Stop"}'
expect_payload "acentos e quebras de linha no pedido" '.message=="“ação com ünïcode em várias linhas”"'
run '{"cwd":"/tmp/x","hook_event_name":"Stop","nested":{"cwd":"/outro"},"title":null}'
expect_payload "pega o primeiro campo e tolera null" '.project=="x"'
}

for tool in jq plutil awk; do
  export QUACKO_JSON="$tool"
  before=$fail
  suite
  echo "  leitor $tool: $([ "$fail" = "$before" ] && echo ok || echo "$((fail - before)) falha(s)")"
done

unset QUACKO_JSON
config() { printf '%s' "$1" >"$QUACKO_CONFIG"; }

rm -f "$QUACKO_CONFIG"
run "" --source ci --status ok
expect_effects "sem config: pausa a música e toca o som" 1 yes
config '{"port":4747,"sound":true,"pause_media":true,"dnd":false}'
run "" --source ci --status fail
expect_effects "tudo ligado: pausa e toca" 1 yes
config '{"sound": false, "pause_media": true}'
run "" --source ci --status ok
expect_effects "som desligado: só pausa" 1 no
expect_payload "som desligado: o cartão aparece igual" '.status=="ok"'
config '{"sound":true,"pause_media":false}'
run "" --source ci --status ok
expect_effects "pausa desligada: só toca" 0 yes
config '{"dnd":true,"sound":true,"pause_media":true}'
run "" --source ci --status ok
expect_effects "não perturbe: nem som nem pausa" 0 no
config '{"sound":false,"pause_media":false}'
run '{"session_id":"cfg","hook_event_name":"UserPromptSubmit","prompt":"x"}' --from claude
expect_no_payload "UserPromptSubmit continua quieto com a config"
config 'não é json'
run "" --source ci --status ok
expect_effects "config inválida: usa o padrão" 1 yes
rm -f "$QUACKO_CONFIG"
echo "  som e pausa pela config: ok"

if command -v node >/dev/null; then
  run_socket() {
    local json="$1"
    shift
    HOOK_JSON="$json" node -e '
      const c = require("child_process").spawn(process.argv[1], process.argv.slice(2), { stdio: ["pipe", "pipe", "pipe"] });
      let out = "", err = "";
      c.stdout.on("data", d => out += d); c.stderr.on("data", d => err += d);
      c.on("close", code => { require("fs").writeFileSync(process.env.OUT_FILE, out); require("fs").writeFileSync(process.env.ERR_FILE, err); process.exit(code); });
      c.stdin.end(process.env.HOOK_JSON);
    ' "$NOTIFY" "$@"
    RC=$?
    OUT=$(cat "$WORK/out")
    PAYLOAD=$(sed -n 's/^payload: //p' "$WORK/err")
  }
  export OUT_FILE="$WORK/out" ERR_FILE="$WORK/err"
  unset QUACKO_JSON
  rm -rf "$WORK/state"
  run_socket '{"session_id":"sock","cwd":"/Users/x/app","hook_event_name":"UserPromptSubmit","prompt":"pedido pelo socket"}' --from claude
  expect_no_payload "socket: UserPromptSubmit só guarda o pedido"
  expect_stdout "socket: UserPromptSubmit não escreve no stdout" ""
  run_socket '{"session_id":"sock","cwd":"/Users/x/app","hook_event_name":"Stop"}' --from claude --sound /System/Library/Sounds/Glass.aiff
  expect_payload "socket: Stop lê o JSON e mostra o pedido" '.status=="ok" and .project=="app" and .message=="“pedido pelo socket”"'
  run_socket '{"hook_event_name":"stop","status":"completed","conversation_id":"c","workspace_roots":["/x/y"]}'
  expect_stdout "socket: Cursor stop responde {}" '{}'
  echo "  stdin por socket: ok"
else
  echo "  stdin por socket: pulado (sem node)"
fi

echo
echo "$pass ok, $fail falhas"
[ "$fail" = 0 ]
