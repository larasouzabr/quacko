#!/bin/bash
# Miau: avisa que algo terminou. Pausa a mídia, toca um som e manda o aviso ao app Miau.
#
# Uso:
#   notify.sh [--from claude|cursor] [--sound CAMINHO] [--source agent|claude|ci|custom]
#             [--status ok|fail|attention|info] [--message TEXTO] [--title TEXTO]
#             [--project TEXTO] [--url URL]
#   notify.sh CAMINHO_DO_SOM          (forma antiga: só o som, continua funcionando)
#
# --from escolhe o adaptador que lê o JSON que a ferramenta manda no stdin (hooks do
# Claude Code ou do Cursor). Sem --from, o formato é detectado sozinho pelo
# `hook_event_name`. Argumentos explícitos sempre vencem o que veio do stdin.
#
# Regra de ouro: nunca quebrar nem atrasar quem chamou. Sempre sai com código 0, em
# poucos milissegundos; com o app fechado, o aviso se perde em silêncio. Nada vai para
# o stdout, exceto as respostas que o Cursor espera (o stdout do UserPromptSubmit do
# Claude entraria no contexto do Claude).
#
# Variáveis: MIAU_PORT (4747), MIAU_LOG, MIAU_STATE_DIR, MIAU_DRY_RUN=1 (testes: não
# pausa, não toca, não envia; escreve o aviso no stderr).

PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
PORT="${MIAU_PORT:-4747}"
STATE_DIR="${MIAU_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/miau}"
LOG="${MIAU_LOG:-/tmp/miau-notify.log}"
DRY_RUN="${MIAU_DRY_RUN:-}"
PROMPT_CHARS=100

# stdout e stderr originais ficam nos descritores 3 e 4; o resto vai para o log.
{ : >>"$LOG"; } 2>/dev/null || LOG=/dev/null
exec 3>&1 4>&2 1>>"$LOG" 2>&1

finish() {
  [ -n "$reply" ] && printf '%s\n' "$reply" >&3
  exit 0
}

# ---------- argumentos ----------
from="" sound="" source="" status="" message="" title="" project="" url="" reply=""
if [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; then
  sound="$1" # forma antiga: o primeiro argumento é o som
  shift
fi
while [ $# -gt 0 ]; do
  opt="$1"
  shift
  val=""
  if [ $# -gt 0 ]; then
    val="$1"
  fi
  case "$opt" in
    --from) from="$val" ;;
    --sound) sound="$val" ;;
    --source) source="$val" ;;
    --status) status="$val" ;;
    --message) message="$val" ;;
    --title) title="$val" ;;
    --project) project="$val" ;;
    --url) url="$val" ;;
    *) continue ;; # opção desconhecida: ignora sem consumir valor
  esac
  [ $# -gt 0 ] && shift
done

case "$source" in agent | claude | ci | custom | "") ;; *) source="" ;; esac
case "$status" in ok | fail | attention | info | "") ;; *) status="" ;; esac

# ---------- stdin (JSON dos hooks) ----------
input=""
if [ -p /dev/stdin ] || [ -f /dev/stdin ]; then
  input=$(head -c 1048576)
fi
have_jq=""
command -v jq >/dev/null && have_jq=1

j() {
  [ -n "$have_jq" ] && [ -n "$input" ] || return 0
  printf '%s' "$input" | jq -r "$1" 2>/dev/null
}

event=$(j 'if type == "object" then .hook_event_name // empty else empty end')
if [ -z "$from" ]; then
  case "$event" in
    Stop | StopFailure | SubagentStop | Notification | UserPromptSubmit) from=claude ;;
    stop | beforeSubmitPrompt) from=cursor ;;
    *) [ "$source" = claude ] && from=claude ;; # spec: --source claude com {"cwd": ...}
  esac
fi

# ---------- pedido do usuário, guardado entre o "enviar" e o "terminou" ----------
prompt_file() { # $1 = ferramenta, $2 = id da sessão/conversa
  local id
  id=$(printf '%s' "$2" | tr -cd 'A-Za-z0-9_-' | cut -c1-80)
  [ -n "$id" ] && printf '%s/prompt-%s-%s' "$STATE_DIR" "$1" "$id"
}

save_prompt() { # $1 = arquivo; o texto vem do campo .prompt do stdin
  [ -n "$1" ] && [ -n "$have_jq" ] || return 0
  (umask 077 && mkdir -p "$STATE_DIR") || return 0
  find "$STATE_DIR" -name 'prompt-*' -mtime +1 -delete 2>/dev/null
  # Espaços e quebras de linha viram um espaço; corta em PROMPT_CHARS caracteres.
  printf '%s' "$input" | jq -r --argjson n "$PROMPT_CHARS" '
    (.prompt // "") | gsub("\\s+"; " ") | sub("^ "; "") | sub(" $"; "")
    | if length > $n then .[0:$n - 1] + "…" else . end' >"$1.tmp" 2>/dev/null &&
    (umask 077 && mv "$1.tmp" "$1")
}

take_prompt() { # imprime e apaga o pedido guardado
  [ -n "$1" ] && [ -f "$1" ] || return 0
  cat "$1"
  rm -f "$1"
}

quoted() { [ -n "$1" ] && printf '“%s”' "$1"; }

# ---------- adaptadores ----------
case "$from" in
  claude)
    [ -z "$source" ] && source=agent
    [ -z "$title" ] && title="Claude Code"
    cwd=$(j '.cwd // empty')
    [ -z "$project" ] && [ -n "$cwd" ] && project=$(basename "$cwd")
    pfile=$(prompt_file claude "$(j '.session_id // empty')")
    case "$event" in
      UserPromptSubmit)
        save_prompt "$pfile"
        finish # só guarda o pedido; nada de som nem aviso
        ;;
      Stop | SubagentStop)
        [ -z "$status" ] && status=ok
        [ -z "$message" ] && message=$(quoted "$(take_prompt "$pfile")")
        ;;
      StopFailure)
        [ -z "$status" ] && status=fail
        [ -z "$message" ] && message=$(quoted "$(take_prompt "$pfile")")
        ;;
      Notification)
        if [ -z "$status" ]; then
          case "$(j '.notification_type // empty')" in
            agent_completed) status=ok ;;
            auth_success | elicitation_complete | elicitation_response | quota_* | plugin_notification) status=info ;;
            *) status=attention ;; # permissão, ociosidade, pergunta…
          esac
        fi
        [ -z "$message" ] && message=$(j '.message // empty')
        ;;
    esac
    ;;
  cursor)
    [ -z "$source" ] && source=agent
    [ -z "$title" ] && title="Cursor"
    root=$(j '.workspace_roots[0] // empty')
    [ -z "$project" ] && [ -n "$root" ] && project=$(basename "$root")
    pfile=$(prompt_file cursor "$(j '.conversation_id // empty')")
    case "$event" in
      beforeSubmitPrompt)
        save_prompt "$pfile"
        reply='{"continue":true}' # nunca bloqueia o envio
        finish
        ;;
      stop)
        reply='{}' # nenhuma mensagem de continuação
        case "$(j '.status // empty')" in
          aborted) # a pessoa interrompeu: não avisa
            rm -f "$pfile" 2>/dev/null
            finish
            ;;
          error) [ -z "$status" ] && status=fail ;;
          *) [ -z "$status" ] && status=ok ;;
        esac
        [ -z "$message" ] && message=$(quoted "$(take_prompt "$pfile")")
        ;;
    esac
    ;;
esac

[ -n "$status" ] && [ -z "$source" ] && source=custom
echo "--- $(date '+%F %T') from=${from:-none} event=${event:-none} status=${status:-none}"

# ---------- mídia e som (como sempre foi) ----------
if [ -z "$DRY_RUN" ]; then
  # Em segundo plano: o osascript leva quase 1 s e o hook não pode esperar por ele.
  (
    osascript -e 'if application "Spotify" is running then tell application "Spotify" to pause'
    command -v nowplaying-cli >/dev/null && nowplaying-cli pause
  ) </dev/null >>"$LOG" 2>&1 &
  if [ -z "$sound" ]; then
    case "$status" in
      fail) sound=/System/Library/Sounds/Basso.aiff ;;
      attention | info) sound=/System/Library/Sounds/Ping.aiff ;;
      *) sound=/System/Library/Sounds/Glass.aiff ;;
    esac
  fi
  [ -f "$sound" ] && (afplay "$sound" </dev/null >/dev/null 2>&1 &)
fi

# ---------- aviso para o app ----------
[ -n "$status" ] || finish # sem status (forma antiga sem hook): só som e pausa

if [ -n "$have_jq" ]; then
  payload=$(jq -nc \
    --arg source "$source" --arg status "$status" --arg message "$message" \
    --arg title "$title" --arg project "$project" --arg url "$url" '
    {source: $source, status: $status}
    + (if $message != "" then {message: $message} else {} end)
    + (if $title != "" then {title: $title} else {} end)
    + (if $project != "" then {project: $project} else {} end)
    + (if $url != "" then {url: $url} else {} end)')
else
  # Sem jq, só os campos de lista fechada (seguros para montar à mão).
  payload="{\"source\":\"$source\",\"status\":\"$status\"}"
fi

if [ -n "$DRY_RUN" ]; then
  printf 'payload: %s\n' "$payload" >&4
else
  (curl -s --max-time 1 -o /dev/null -X POST -H 'Content-Type: application/json' \
    --data-binary "$payload" "http://127.0.0.1:$PORT/notify" </dev/null >/dev/null 2>&1 &)
fi
finish
