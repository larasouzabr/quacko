#!/bin/bash

PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
PORT="${QUACKO_PORT:-4747}"
STATE_DIR="${QUACKO_STATE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/quacko}"
LOG="${QUACKO_LOG:-/tmp/quacko-notify.log}"
DRY_RUN="${QUACKO_DRY_RUN:-}"
CONFIG="${QUACKO_CONFIG:-$HOME/.config/quacko/config.json}"
PROMPT_CHARS=100
export LC_ALL=en_US.UTF-8

{ : >>"$LOG"; } 2>/dev/null || LOG=/dev/null
exec 3>&1 4>&2 1>>"$LOG" 2>&1

finish() {
  [ -n "$reply" ] && printf '%s\n' "$reply" >&3
  exit 0
}

from="" sound="" source="" status="" message="" title="" project="" url="" reply="" sticky=""
if [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; then
  sound="$1"
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
    --sticky)
      sticky=1
      continue
      ;;
    --from) from="$val" ;;
    --sound) sound="$val" ;;
    --source) source="$val" ;;
    --status) status="$val" ;;
    --message) message="$val" ;;
    --title) title="$val" ;;
    --project) project="$val" ;;
    --url) url="$val" ;;
    *) continue ;;
  esac
  [ $# -gt 0 ] && shift
done

case "$source" in agent | claude | ci | custom | "") ;; *) source="" ;; esac
case "$status" in ok | fail | attention | info | "") ;; *) status="" ;; esac

input=""
if [ -p /dev/stdin ] || [ -f /dev/stdin ] || [ -S /dev/stdin ]; then
  input=$(head -c 1048576)
fi

JSON_TOOL="${QUACKO_JSON:-}"
if [ -z "$JSON_TOOL" ]; then
  if command -v jq >/dev/null; then
    JSON_TOOL=jq
  elif [ "$(printf '{"a":"b"}' | plutil -extract a raw -o - - 2>/dev/null)" = b ]; then
    JSON_TOOL=plutil
  else
    JSON_TOOL=awk
  fi
fi

j() {
  [ -n "$input" ] || return 0
  case "$JSON_TOOL" in
    jq)
      printf '%s' "$input" | jq -r --arg p "$1" '
        if type == "object" then
          (try getpath($p | split(".") | map(if test("^[0-9]+$") then tonumber else . end)) catch null)
          | if type == "string" or type == "number" or type == "boolean" then tostring else empty end
        else empty end' 2>/dev/null
      ;;
    plutil)
      printf '%s' "$input" | plutil -extract "$1" raw -o - - 2>/dev/null
      ;;
    *) json_awk "$1" ;;
  esac
}

json_awk() {
  local key="${1%%.*}" arr=""
  [ "$key" != "$1" ] && arr=1
  printf '%s' "$input" | awk -v key="$key" -v arr="$arr" '
    BEGIN { RS = "\001" }
    {
      s = $0; k = "\"" key "\""
      while ((i = index(s, k)) > 0) {
        s = substr(s, i + length(k))
        if (!match(s, /^[ \t\r\n]*:[ \t\r\n]*/)) continue
        s = substr(s, RLENGTH + 1)
        if (arr) { if (!match(s, /^\[[ \t\r\n]*/)) exit; s = substr(s, RLENGTH + 1) }
        if (substr(s, 1, 1) != "\"") exit
        out = ""; n = length(s); p = 2
        while (p <= n) {
          c = substr(s, p, 1)
          if (c == "\\") {
            d = substr(s, p + 1, 1); p += 2
            if (d == "n") out = out "\n"; else if (d == "t") out = out "\t"
            else if (d == "r") out = out "\r"; else if (d == "b" || d == "f") out = out
            else if (d == "u") { out = out "\\u" substr(s, p, 4); p += 4 }
            else out = out d
            continue
          }
          if (c == "\"") { printf "%s", out; exit }
          out = out c; p++
        }
        exit
      }
    }' 2>/dev/null
}

json_str() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\r'/\\r}"
  s="${s//$'\t'/\\t}"
  s=$(printf '%s' "$s" | tr -d '\000-\010\013\014\016-\037')
  printf '"%s"' "$s"
}

one_line() {
  local p
  p=$(printf '%s' "$1" | tr '\n\r\t' '   ' | tr -s ' ')
  p="${p#"${p%%[! ]*}"}"
  p="${p%"${p##*[! ]}"}"
  if [ "${#p}" -gt "$PROMPT_CHARS" ]; then
    p="${p:0:$((PROMPT_CHARS - 1))}…"
  fi
  printf '%s' "$p"
}

event=$(j hook_event_name)
if [ -z "$from" ]; then
  case "$event" in
    Stop | StopFailure | SubagentStop | Notification | UserPromptSubmit) from=claude ;;
    stop | beforeSubmitPrompt) from=cursor ;;
    *) [ "$source" = claude ] && from=claude ;;
  esac
fi

prompt_file() {
  local id
  id=$(printf '%s' "$2" | tr -cd 'A-Za-z0-9_-' | cut -c1-80)
  [ -n "$id" ] && printf '%s/prompt-%s-%s' "$STATE_DIR" "$1" "$id"
}

save_prompt() {
  [ -n "$1" ] || return 0
  (umask 077 && mkdir -p "$STATE_DIR") || return 0
  find "$STATE_DIR" -name 'prompt-*' -mtime +1 -delete 2>/dev/null
  (umask 077 && one_line "$(j prompt)" >"$1.tmp" && mv "$1.tmp" "$1")
}

take_prompt() {
  [ -n "$1" ] && [ -f "$1" ] || return 0
  cat "$1"
  rm -f "$1"
}

quoted() { [ -n "$1" ] && printf '“%s”' "$1"; }

case "$from" in
  claude)
    [ -z "$source" ] && source=agent
    [ -z "$title" ] && title="Claude Code"
    cwd=$(j cwd)
    [ -z "$project" ] && [ -n "$cwd" ] && project=$(basename "$cwd")
    pfile=$(prompt_file claude "$(j session_id)")
    case "$event" in
      UserPromptSubmit)
        save_prompt "$pfile"
        finish
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
          case "$(j notification_type)" in
            agent_completed) status=ok ;;
            auth_success | elicitation_complete | elicitation_response | quota_* | plugin_notification) status=info ;;
            *) status=attention ;;
          esac
        fi
        [ -z "$message" ] && message=$(one_line "$(j message)")
        ;;
    esac
    ;;
  cursor)
    [ -z "$source" ] && source=agent
    [ -z "$title" ] && title="Cursor"
    root=$(j workspace_roots.0)
    [ -z "$project" ] && [ -n "$root" ] && project=$(basename "$root")
    pfile=$(prompt_file cursor "$(j conversation_id)")
    case "$event" in
      beforeSubmitPrompt)
        save_prompt "$pfile"
        reply='{"continue":true}'
        finish
        ;;
      stop)
        reply='{}'
        case "$(j status)" in
          aborted)
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

detach() {
  if [ -x /usr/bin/perl ]; then
    /usr/bin/perl -MPOSIX -e 'fork and exit; POSIX::setsid(); exec @ARGV or exit 1' "$@" </dev/null >>"$LOG" 2>&1
  else
    ("$@" </dev/null >>"$LOG" 2>&1 &)
  fi
}

setting_is() {
  [ -f "$CONFIG" ] && grep -Eq "\"$1\"[[:space:]]*:[[:space:]]*$2" "$CONFIG" 2>/dev/null
}

pause=1
play=1
if setting_is dnd true; then
  pause=""
  play=""
fi
setting_is pause_media false && pause=""
setting_is sound false && play=""

if [ -z "$sound" ]; then
  case "$status" in
    fail) sound=/System/Library/Sounds/Basso.aiff ;;
    attention | info) sound=/System/Library/Sounds/Ping.aiff ;;
    *) sound=/System/Library/Sounds/Glass.aiff ;;
  esac
fi
[ -n "$play" ] || sound=""

if [ -n "$DRY_RUN" ]; then
  printf 'effects: pause=%s sound=%s\n' "${pause:-0}" "$sound" >&4
else
  [ -n "$pause" ] && detach /bin/bash -c '
    osascript -e "if application \"Spotify\" is running then tell application \"Spotify\" to pause"
    command -v nowplaying-cli >/dev/null && nowplaying-cli pause'
  [ -n "$sound" ] && [ -f "$sound" ] && detach /usr/bin/afplay "$sound"
fi

[ -n "$status" ] || finish

if [ "$JSON_TOOL" = jq ]; then
  payload=$(jq -nc \
    --arg source "$source" --arg status "$status" --arg message "$message" \
    --arg title "$title" --arg project "$project" --arg url "$url" --arg sticky "$sticky" '
    {source: $source, status: $status}
    + (if $message != "" then {message: $message} else {} end)
    + (if $title != "" then {title: $title} else {} end)
    + (if $project != "" then {project: $project} else {} end)
    + (if $url != "" then {url: $url} else {} end)
    + (if $sticky != "" then {sticky: true} else {} end)')
else
  payload="{\"source\":\"$source\",\"status\":\"$status\""
  [ -n "$message" ] && payload="$payload,\"message\":$(json_str "$message")"
  [ -n "$title" ] && payload="$payload,\"title\":$(json_str "$title")"
  [ -n "$project" ] && payload="$payload,\"project\":$(json_str "$project")"
  [ -n "$url" ] && payload="$payload,\"url\":$(json_str "$url")"
  [ -n "$sticky" ] && payload="$payload,\"sticky\":true"
  payload="$payload}"
fi

if [ -n "$DRY_RUN" ]; then
  printf 'payload: %s\n' "$payload" >&4
else
  (curl -s --max-time 1 -o /dev/null -X POST -H 'Content-Type: application/json' \
    --data-binary "$payload" "http://127.0.0.1:$PORT/notify" </dev/null >/dev/null 2>&1 &)
fi
finish
