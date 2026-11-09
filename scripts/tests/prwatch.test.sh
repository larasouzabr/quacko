#!/bin/bash
# Testes do prwatch com um `gh` falso e um notify.sh falso que só registra os argumentos.
# Uso: bash scripts/tests/prwatch.test.sh

HERE="$(cd "$(dirname "$0")" && pwd)"
PRWATCH="$HERE/../prwatch"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bin"

# gh falso: `checks` sai com $FAKE_RC; `view` devolve $FAKE_VIEW (ou falha se vazio).
cat >"$WORK/bin/gh" <<'GH'
#!/bin/bash
echo "$*" >>"$FAKE_GH_LOG"
case "$2" in
  checks) exit "${FAKE_RC:-0}" ;;
  view) [ -n "$FAKE_VIEW" ] && printf '%s' "$FAKE_VIEW" && exit 0; exit 1 ;;
esac
GH
cat >"$WORK/bin/notify" <<'NT'
#!/bin/bash
printf '%s\n' "$@" >"$FAKE_NOTIFY_LOG"
NT
chmod +x "$WORK/bin/gh" "$WORK/bin/notify"
export PATH="$WORK/bin:$PATH" MIAU_NOTIFY="$WORK/bin/notify"
export FAKE_GH_LOG="$WORK/gh.log" FAKE_NOTIFY_LOG="$WORK/notify.log"

pass=0
fail=0
check() { if eval "$2"; then pass=$((pass + 1)); else fail=$((fail + 1)); echo "FALHOU: $1"; fi; }
args() { cat "$FAKE_NOTIFY_LOG" 2>/dev/null | tr '\n' '|'; }
reset() { rm -f "$FAKE_GH_LOG" "$FAKE_NOTIFY_LOG"; }

VIEW='{"number":1234,"title":"Corrige fuso horário no parser","url":"https://github.com/pondworks/lagoa-api/pull/1234"}'

reset
FAKE_RC=1 FAKE_VIEW="$VIEW" "$PRWATCH" 1234 --repo pondworks/lagoa-api --required
rc=$?
check "devolve o código do gh pr checks" '[ $rc = 1 ]'
check "falha avisa com status fail e som Basso" '[[ "$(args)" == *"--status|fail|"* && "$(args)" == *"Basso.aiff"* ]]'
check "resultado fica até fechar no × (--sticky)" '[[ "$(args)" == *"--sticky|"* ]]'
check "título PR #N" '[[ "$(args)" == *"--title|PR #1234|"* ]]'
check "mensagem = título do PR (qual PR falhou)" '[[ "$(args)" == *"--message|Corrige fuso horário no parser|"* ]]'
check "projeto = dono/repo#N" '[[ "$(args)" == *"--project|pondworks/lagoa-api#1234|"* ]]'
check "url do PR" '[[ "$(args)" == *"--url|https://github.com/pondworks/lagoa-api/pull/1234|"* ]]'
check "checks recebe --watch --interval 15" 'grep -q "pr checks 1234 --repo pondworks/lagoa-api --required --watch --interval 15" "$FAKE_GH_LOG"'
check "view recebe só o PR e o --repo" 'grep -qx "pr view 1234 --repo pondworks/lagoa-api --json number,title,url" "$FAKE_GH_LOG"'

reset
FAKE_RC=0 FAKE_VIEW="$VIEW" "$PRWATCH"
check "sucesso avisa com ok e Glass" '[[ "$(args)" == *"--status|ok|"* && "$(args)" == *"Glass.aiff"* ]]'
check "sem argumentos: view sem argumentos" 'grep -qx "pr view --json number,title,url" "$FAKE_GH_LOG"'

reset
FAKE_RC=1 FAKE_VIEW="" "$PRWATCH" 99
check "sem dados do PR: avisa genérico (sem título/mensagem)" '[[ "$(args)" == *"--status|fail|"* && "$(args)" != *"--title"* ]]'

reset
FAKE_RC=130 FAKE_VIEW="$VIEW" "$PRWATCH" 1
rc=$?
check "Ctrl+C não avisa e devolve 130" '[ $rc = 130 ] && [ ! -f "$FAKE_NOTIFY_LOG" ]'

echo
echo "$pass ok, $fail falhas"
[ "$fail" = 0 ]
