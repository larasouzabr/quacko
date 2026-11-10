[[ $- == *i* ]] || return 0
[ -n "${_QUACKO_SHELL:-}" ] && return 0
_QUACKO_SHELL=1
_quacko_cmd=""
_quacko_start=0
_quacko_armed=""
_quacko_rc=0

_quacko_interactive=" vi vim nvim nano emacs micro hx helix less more most man info top htop btop glances ssh mosh telnet tmux screen zellij claude cursor-agent codex aider gemini ipython watch tail fzf lazygit tig ranger nnn yazi "
_quacko_repls=" python python3 node irb pry psql mysql sqlite3 redis-cli mongosh bash zsh fish sh "

_quacko_quiet() {
  local -a words
  read -r -a words <<<"$1"
  while [ "${#words[@]}" -gt 0 ]; do
    case "${words[0]}" in
      *=* | sudo | time | nohup | command | builtin | exec | env) words=("${words[@]:1}") ;;
      *) break ;;
    esac
  done
  [ "${#words[@]}" -gt 0 ] || return 0
  local program="${words[0]##*/}"
  [[ $_quacko_interactive == *" $program "* ]] && return 0
  [[ " ${QUACKO_IGNORE:-} " == *" $program "* ]] && return 0
  [[ $_quacko_repls == *" $program "* && ${#words[@]} -eq 1 ]] && return 0
  return 1
}

_quacko_duration() {
  local s=$1
  if [ "$s" -lt 60 ]; then
    echo "${s}s"
  elif [ "$s" -lt 3600 ]; then
    echo "$((s / 60))m $((s % 60))s"
  else
    echo "$((s / 3600))h $((s % 3600 / 60))m"
  fi
}

_quacko_preexec() {
  [ -n "$_quacko_armed" ] || return 0
  [ -n "${COMP_LINE:-}" ] && return 0
  _quacko_armed=""
  case "$BASH_COMMAND" in _quacko_save_rc*) return 0 ;; esac
  _quacko_cmd="${1:-$BASH_COMMAND}"
  if [ -z "${1:-}" ]; then
    local line number
    line=$(HISTTIMEFORMAT='' builtin history 1 2>/dev/null)
    line="${line#"${line%%[![:space:]]*}"}"
    number="${line%%[!0-9]*}"
    line="${line#"$number"}"
    line="${line#"${line%%[![:space:]]*}"}"
    if [ -n "$number" ] && [ "$number" != "${_quacko_histnum:-}" ] && [ -n "$line" ]; then
      _quacko_cmd="$line"
      _quacko_histnum=$number
    fi
  fi
  _quacko_start=$SECONDS
}

_quacko_precmd() {
  local rc=${1:-$_quacko_rc} cmd=$_quacko_cmd
  _quacko_cmd=""
  [ -n "$cmd" ] || return 0
  local secs=$((SECONDS - _quacko_start))
  [ "$secs" -ge "${QUACKO_MIN_SECS:-30}" ] || return 0
  case $rc in 130 | 131 | 146 | 148) return 0 ;; esac
  _quacko_quiet "$cmd" && return 0

  local notify=${QUACKO_NOTIFY:-$HOME/.claude/hooks/notify.sh}
  [ -x "$notify" ] || return 0
  local title=${cmd//$'\n'/ }
  title=${title//$'\t'/ }
  [ "${#title}" -gt 60 ] && title="${title:0:59}…"
  local took
  took=$(_quacko_duration "$secs")
  local -a args=(--from terminal --source custom --title "$title" --project "${PWD##*/}")
  if [ "$rc" -eq 0 ]; then
    args+=(--status ok --message "$took")
  else
    args+=(--status fail --message "exit $rc · $took" --sticky)
  fi
  ("$notify" "${args[@]}" </dev/null >/dev/null 2>&1 &)
  return 0
}

if [ -n "${bash_preexec_imported:-}${__bp_imported:-}" ]; then
  _quacko_bp_preexec() { _quacko_armed=1; _quacko_preexec "$1"; }
  _quacko_bp_precmd() { _quacko_precmd "$?"; }
  preexec_functions+=(_quacko_bp_preexec)
  precmd_functions=(_quacko_bp_precmd "${precmd_functions[@]}")
else
  read -r -d '' _quacko_setup <<'QUACKO_SETUP'
if [ -z "${_quacko_trap_on:-}" ]; then
  _quacko_trap_on=1
  _quacko_previous_debug=$(trap -p DEBUG)
  _quacko_previous_debug=${_quacko_previous_debug#"trap -- '"}
  _quacko_previous_debug=${_quacko_previous_debug%"' DEBUG"}
  if [ -n "$_quacko_previous_debug" ]; then
    trap '_quacko_preexec; eval "$_quacko_previous_debug"' DEBUG
  else
    trap '_quacko_preexec' DEBUG
  fi
fi
_quacko_armed=1
QUACKO_SETUP
  _quacko_save_rc() {
    _quacko_rc=$?
  }
  if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(_quacko_save_rc "${PROMPT_COMMAND[@]}" _quacko_precmd 'eval "$_quacko_setup"')
  else
    PROMPT_COMMAND="_quacko_save_rc${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _quacko_precmd; eval \"\$_quacko_setup\""
  fi
fi
