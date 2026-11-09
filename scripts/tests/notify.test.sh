#!/bin/bash
# Testes do notify.sh em modo seco (QUACKO_DRY_RUN=1): não pausa mídia, não toca som,
# não envia nada. Confere o aviso gerado, o stdout e o código de saída.
#
# A suíte roda uma vez para cada leitor de JSON do script: jq (preferido), plutil e awk
# (os caminhos sem jq). O leitor é forçado com QUACKO_JSON porque no macOS 15+ o jq vem em
# /usr/bin, junto com bash/awk/sed, e não dá para tirá-lo do PATH só para o teste.
# Os testes usam o jq para conferir o JSON gerado (isso é do teste, não do script).
# Uso: bash scripts/tests/notify.test.sh

HERE="$(cd "$(dirname "$0")" && pwd)"
NOTIFY="$HERE/../notify.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export QUACKO_DRY_RUN=1 QUACKO_STATE_DIR="$WORK/state" QUACKO_LOG="$WORK/log"

pass=0
fail=0
ok() { pass=$((pass + 1)); }
bad() {
  fail=$((fail + 1))
  echo "FALHOU: $1"
  [ -n "${2:-}" ] && echo "   $2"
}

# run "<stdin>" args...  -> preenche $OUT (stdout), $PAYLOAD (json ou vazio), $RC
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
}

expect_payload() { # nome, filtro jq que deve dar true
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
# --- aceite da spec (Marco 5) ---
run "" --source ci --status fail --message "PR #1: checks falharam" --project teste
expect_payload "flags explícitas" '.source=="ci" and .status=="fail" and .message=="PR #1: checks falharam" and .project=="teste"'
expect_stdout "flags explícitas: stdout vazio" ""

run '{"cwd":"/tmp/meu-projeto"}' --source claude --status ok
expect_payload "cwd vira project" '.status=="ok" and .project=="meu-projeto"'

# --- Claude Code ---
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

# Forma antiga (só o som) com o JSON do hook: detecta o Claude sozinho.
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

# Sessões diferentes não misturam pedidos.
run '{"session_id":"s1","hook_event_name":"UserPromptSubmit","prompt":"pedido um"}'
run '{"session_id":"s2","hook_event_name":"UserPromptSubmit","prompt":"pedido dois"}'
run '{"session_id":"s1","hook_event_name":"Stop"}'
expect_payload "pedido certo por sessão" '.message=="“pedido um”"'

# Pedido longo é cortado; aspas, emoji e caracteres especiais viram JSON válido.
LONG=$(printf 'a%.0s' $(seq 1 300))
run "{\"session_id\":\"s3\",\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"$LONG\"}"
run '{"session_id":"s3","hook_event_name":"Stop"}'
expect_payload "pedido longo cortado em 100 caracteres" '(.message | length) == 102 and (.message | endswith("…”"))'
run '{"session_id":"s4","hook_event_name":"UserPromptSubmit","prompt":"diga \"oi\" $(rm -rf /) `x` \\\\ 🐱"}'
run '{"session_id":"s4","hook_event_name":"Stop"}'
expect_payload "caracteres especiais preservados como texto" '.message=="“diga \"oi\" $(rm -rf /) `x` \\\\ 🐱”"'

# Id de sessão com caminho malicioso não escapa da pasta de estado.
run '{"session_id":"../../etc/x","hook_event_name":"UserPromptSubmit","prompt":"p"}'
[ ! -e "$WORK/etc" ] && ls "$WORK/state" | grep -q '^prompt-claude-etcx$' && ok || bad "session_id higienizado"

# --- Cursor ---
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

# --- robustez ---
run 'isso não é json' --from claude
expect_rc0 "stdin inválido sai com 0"
expect_no_payload "stdin inválido sem status: nada"
run "" --status talvez --source hacker
expect_no_payload "status/source fora da lista são ignorados"
run "" --status info --message
expect_payload "flag sem valor no fim não trava" '.status=="info" and .source=="custom"'
run "" --desconhecida --status ok
expect_payload "opção desconhecida é ignorada" '.status=="ok"'

# --- --sticky (sem valor) ---
run "" --source ci --status fail --sticky --title "PR #1"
expect_payload "--sticky marca o cartão" '.sticky == true and .title == "PR #1"'
run "" --source ci --status ok
expect_payload "sem --sticky não manda o campo" '(has("sticky") | not)'

# --- caminho sem jq: textos com caracteres de controle viram JSON válido ---
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

# --- stdin por socket (como o Claude Code, em Node, entrega o JSON do hook) ---
# Os testes acima usam pipe; foi com socket que o script deixou de ler o hook.
if command -v node >/dev/null; then
  run_socket() { # "<json>" args... (stdin vira um socket, como num spawn do Node)
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
