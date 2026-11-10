#!/bin/bash

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/../quacko.zsh"
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
check() { if eval "$2"; then pass=$((pass + 1)); else fail=$((fail + 1)); echo "FALHOU: $1"; fi; }
calls() { cat "$LOG" 2>/dev/null; }
count() { calls | grep -c . | tr -d ' '; }

session() {
  rm -f "$LOG"
  {
    printf 'boom() { return "${1:-1}" }\n'
    printf 'vim() { return 0 }\n'
    printf 'meutool() { return 1 }\n'
    printf 'python3() { return 1 }\n'
    printf 'source %q\n' "$SCRIPT"
    printf '%s\n' "$@"
  } | (cd "$WORK/projeto-lagoa" && HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" \
    QUACKO_MIN_SECS="${MIN:-0}" QUACKO_IGNORE="${IGNORE:-}" zsh -f -i >/dev/null 2>&1)
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [ "$(count)" -ge "${WANT:-1}" ] && break
    sleep 0.1
  done
}

session 'boom 2'
check "falha avisa com status fail" '[[ "$(calls)" == *"--status|fail|"* ]]'
check "falha mostra o código de saída e o tempo" '[[ "$(calls)" == *"--message|exit 2 · 0s|"* ]]'
check "falha fica na tela até fechar (--sticky)" '[[ "$(calls)" == *"--sticky|"* ]]'
check "título é o comando" '[[ "$(calls)" == *"--title|boom 2|"* ]]'
check "projeto é a pasta atual" '[[ "$(calls)" == *"--project|projeto-lagoa|"* ]]'
check "origem custom" '[[ "$(calls)" == *"--source|custom|"* ]]'

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

durations=$(zsh -f -i -c "source $(printf %q "$SCRIPT"); _quacko_duration 45; _quacko_duration 134; _quacko_duration 3780" 2>/dev/null | tr '\n' '|')
check "tempo em s, m s e h m" '[ "$durations" = "45s|2m 14s|1h 3m|" ]'

rm -f "$LOG"
HOME="$WORK" FAKE_NOTIFY_LOG="$LOG" QUACKO_NOTIFY="$WORK/notify" QUACKO_MIN_SECS=0 \
  zsh -f -c "source $(printf %q "$SCRIPT"); false; true" >/dev/null 2>&1
sleep 0.3
check "shell não interativo (scripts) não avisa" '[ "$(count)" = 0 ]'

rm -f "$LOG"
out=$(printf 'source %q\nfalse\n' "$SCRIPT" | HOME="$WORK" QUACKO_NOTIFY="$WORK/nao-existe" QUACKO_MIN_SECS=0 zsh -f -i 2>&1)
check "sem o notify.sh não mostra erro" '[[ "$out" != *"nao-existe"* && "$out" != *"not found"* ]]'

echo
echo "$pass ok, $fail falhas"
[ "$fail" = 0 ]
