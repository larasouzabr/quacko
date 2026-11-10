#!/bin/bash

HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/projeto-lagoa"

cat >"$WORK/notify" <<'NT'
#!/bin/bash
printf '%s|' "$@" >>"$FAKE_NOTIFY_LOG"
echo >>"$FAKE_NOTIFY_LOG"
NT
chmod +x "$WORK/notify"
LOG="$WORK/notify.log"

pass=0
fail=0
check() { if eval "$2"; then pass=$((pass + 1)); else fail=$((fail + 1)); echo "FALHOU ($NAME): $1"; fi; }
calls() { cat "$LOG" 2>/dev/null; }
count() { calls | grep -c . | tr -d ' '; }

session() {
  rm -f "$LOG"
  {
    printf 'boom() { return "${1:-1}"; }\n'
    printf 'vim() { return 0; }\n'
    printf 'meutool() { return 1; }\n'
    printf 'python3() { return 1; }\n'
    printf '%s\n' "${SETUP:-}"
    printf 'source %q\n' "$SCRIPT"
    printf '%s\n' "$@"
  } | (cd "$WORK/projeto-lagoa" && HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" \
    QUACKO_MIN_SECS="${MIN:-0}" QUACKO_IGNORE="${IGNORE:-}" "${INTERACTIVE[@]}" >/dev/null 2>&1)
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [ "$(count)" -ge "${WANT:-1}" ] && break
    sleep 0.1
  done
}

suite() {
  session 'boom 2'
  check "falha avisa com status fail" '[[ "$(calls)" == *"--status|fail|"* ]]'
  check "falha mostra o código de saída e o tempo" '[[ "$(calls)" == *"--message|exit 2 · 0s|"* ]]'
  check "falha fica na tela até fechar (--sticky)" '[[ "$(calls)" == *"--sticky|"* ]]'
  check "título é o comando" '[[ "$(calls)" == *"--title|boom 2|"* ]]'
  check "projeto é a pasta atual" '[[ "$(calls)" == *"--project|projeto-lagoa|"* ]]'
  check "origem custom, vindo do terminal" '[[ "$(calls)" == *"--from|terminal|--source|custom|"* ]]'

  session 'boom 0'
  check "sucesso avisa com status ok" '[[ "$(calls)" == *"--status|ok|"* && "$(calls)" == *"--message|0s|"* ]]'
  check "sucesso some sozinho (sem --sticky)" '[[ "$(calls)" != *"--sticky"* ]]'

  MIN=60 WANT=0 session 'boom 1'
  check "comando rápido não avisa" '[ "$(count)" = 0 ]'

  WANT=0 session 'boom 130' 'boom 146' 'boom 148'
  check "Ctrl+C e Ctrl+Z não avisam" '[ "$(count)" = 0 ]'

  WANT=0 session 'vim notas.txt' 'FOO=1 sudo vim x' 'command vim'
  check "programas interativos não avisam" '[ "$(count)" = 0 ]'

  IGNORE="outro meutool" WANT=0 session 'meutool --rápido'
  check "QUACKO_IGNORE soma programas à lista" '[ "$(count)" = 0 ]'

  WANT=0 session 'python3' 'NODE_ENV=x python3'
  check "REPL sem argumentos não avisa" '[ "$(count)" = 0 ]'

  session 'python3 treino.py --epocas 3'
  check "script rodado pelo python avisa" '[[ "$(calls)" == *"--title|python3 treino.py --epocas 3|"* ]]'

  IGNORE="python3" WANT=0 session 'python3 treino.py'
  check "QUACKO_IGNORE soma programas à lista" '[ "$(count)" = 0 ]'

  WANT=0 session ''
  check "linha vazia não avisa" '[ "$(count)" = 0 ]'

  WANT=2 session 'boom 1' 'boom 0'
  check "um aviso por comando" '[ "$(count)" = 2 ]'

  WANT=2 session "source $(printf %q "$SCRIPT")" 'boom 1'
  check "carregar duas vezes não duplica o aviso" 'sleep 0.3; [ "$(count)" = 2 ]'

  session 'boom 1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  title=$(calls | sed -E 's/.*--title\|([^|]*)\|.*/\1/')
  check "título longo é cortado em 60 caracteres com …" '[ "$(printf "%s" "$title" | LC_ALL=en_US.UTF-8 wc -m | tr -d " ")" = 60 ] && [[ "$title" == *… ]]'

  durations=$("${INTERACTIVE[@]}" -c "source $(printf %q "$SCRIPT"); _quacko_duration 45; _quacko_duration 134; _quacko_duration 3780" 2>/dev/null | tr '\n' '|')
  check "tempo em s, m s e h m" '[ "$durations" = "45s|2m 14s|1h 3m|" ]'

  rm -f "$LOG"
  HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" QUACKO_MIN_SECS=0 \
    "${PLAIN[@]}" -c "source $(printf %q "$SCRIPT"); false; true" >/dev/null 2>&1
  sleep 0.3
  check "shell não interativo (scripts) não avisa" '[ "$(count)" = 0 ]'

  rm -f "$LOG"
  out=$(printf 'source %q\nfalse\n' "$SCRIPT" | HOME="$WORK" QUACKO_NOTIFY="$WORK/nao-existe" QUACKO_MIN_SECS=0 "${INTERACTIVE[@]}" 2>&1)
  check "sem o notify.sh não mostra erro" '[[ "$out" != *"nao-existe"* && "$out" != *"not found"* ]]'

  SETUP='PROMPT_COMMAND_TEST=1' WANT=1 session 'boom 3'
  check "funciona com outras configurações do shell" '[[ "$(calls)" == *"exit 3"* ]]'
}

NAME=zsh SCRIPT="$HERE/../quacko.zsh"
INTERACTIVE=(zsh -f -i) PLAIN=(zsh -f)
suite

for candidate in /bin/bash ${QUACKO_BASH:-}; do
  [ -x "$candidate" ] || continue
  NAME="bash $("$candidate" -c 'echo $BASH_VERSION')" SCRIPT="$HERE/../quacko.bash"
  INTERACTIVE=("$candidate" --norc --noprofile -i) PLAIN=("$candidate" --norc --noprofile)
  suite
done

rm -f "$LOG"
printf '%s\n' 'PROMPT_COMMAND="echo meu-prompt >>$PC_LOG"' "trap 'echo meu-debug >>\$PC_LOG' DEBUG" \
  "source $(printf %q "$HERE/../quacko.bash")" 'boom() { return 4; }' 'boom' |
  (HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" QUACKO_MIN_SECS=0 PC_LOG="$WORK/pc.log" \
    /bin/bash --norc --noprofile -i >/dev/null 2>&1)
sleep 0.3
NAME=bash
check "bash: mantém o PROMPT_COMMAND de antes" 'grep -q meu-prompt "$WORK/pc.log"'
check "bash: mantém o trap DEBUG de antes" 'grep -q meu-debug "$WORK/pc.log"'
check "bash: com PROMPT_COMMAND e trap próprios, ainda avisa" '[[ "$(calls)" == *"exit 4"* ]]'

cat >"$WORK/fake-bash-preexec.sh" <<'BP'
__bp_imported=1
preexec_functions=()
precmd_functions=()
__fake_armed=1
__fake_debug() {
  [ -n "$__fake_armed" ] || return 0
  case "$BASH_COMMAND" in __fake_precmd*) return 0 ;; esac
  __fake_armed=""
  local f
  for f in "${preexec_functions[@]}"; do "$f" "$BASH_COMMAND"; done
}
__fake_precmd() {
  local rc=$? f
  for f in "${precmd_functions[@]}"; do (exit $rc); "$f"; done
  __fake_armed=1
}
trap '__fake_debug' DEBUG
PROMPT_COMMAND=__fake_precmd
BP
rm -f "$LOG"
printf '%s\n' 'boom() { return "${1:-1}"; }' 'vim() { return 0; }' "source $(printf %q "$WORK/fake-bash-preexec.sh")" \
  "source $(printf %q "$HERE/../quacko.bash")" 'boom 5' '' 'vim x' 'boom 0' |
  (HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" QUACKO_MIN_SECS=0 /bin/bash --norc --noprofile -i >/dev/null 2>&1)
sleep 0.3
check "bash-preexec: usa as listas dele" '[[ "$(calls)" == *"--title|boom 5|"*"exit 5"* ]]'
check "bash-preexec: um aviso por comando, sem o vim" '[ "$(count)" = 2 ]'

echo
echo "$pass ok, $fail falhas"
[ "$fail" = 0 ]
