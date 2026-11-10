[[ -o interactive ]] || return 0
(( ${+_QUACKO_SHELL} )) && return 0
zmodload zsh/datetime 2>/dev/null || return 0
autoload -Uz add-zsh-hook
typeset -g _QUACKO_SHELL=1 _quacko_cmd="" _quacko_start=0

typeset -ga _quacko_interactive=(
  vi vim nvim nano emacs micro hx helix less more most man info
  top htop btop glances ssh mosh telnet tmux screen zellij
  claude cursor-agent codex aider gemini ipython
  watch tail fzf lazygit tig ranger nnn yazi
)
typeset -ga _quacko_repls=(
  python python3 node irb pry psql mysql sqlite3 redis-cli mongosh bash zsh fish sh
)

_quacko_words() {
  local -a words=(${(z)1})
  while (( $#words )) && [[ $words[1] == *=* || $words[1] == (sudo|time|nohup|command|builtin|exec|noglob|nocorrect|env) ]]; do
    shift words
  done
  (( $#words )) && words[1]=${words[1]:t}
  reply=($words)
}

_quacko_quiet() {
  local -a reply
  _quacko_words "$1"
  local program=$reply[1]
  [[ -z $program ]] && return 0
  (( ${_quacko_interactive[(Ie)$program]} )) && return 0
  (( ${${=QUACKO_IGNORE:-}[(Ie)$program]} )) && return 0
  (( ${_quacko_repls[(Ie)$program]} && $#reply == 1 )) && return 0
  return 1
}

_quacko_duration() {
  local s=$1
  if (( s < 60 )); then
    print -r -- "${s}s"
  elif (( s < 3600 )); then
    print -r -- "$(( s / 60 ))m $(( s % 60 ))s"
  else
    print -r -- "$(( s / 3600 ))h $(( s % 3600 / 60 ))m"
  fi
}

_quacko_preexec() {
  _quacko_cmd=$1
  _quacko_start=$EPOCHSECONDS
}

_quacko_precmd() {
  local rc=$? cmd=$_quacko_cmd
  _quacko_cmd=""
  [[ -n $cmd ]] || return 0
  local secs=$(( EPOCHSECONDS - _quacko_start ))
  (( secs >= ${QUACKO_MIN_SECS:-30} )) || return 0
  case $rc in 130 | 131 | 146 | 148) return 0 ;; esac
  _quacko_quiet "$cmd" && return 0

  local notify=${QUACKO_NOTIFY:-$HOME/.claude/hooks/notify.sh}
  [[ -x $notify ]] || return 0
  local title=${${cmd//$'\n'/ }//$'\t'/ }
  (( ${#title} > 60 )) && title="${title[1,59]}…"
  local took=$(_quacko_duration $secs)
  local -a args=(--source custom --title "$title" --project "${PWD:t}")
  if (( rc == 0 )); then
    args+=(--status ok --message "$took")
  else
    args+=(--status fail --message "exit $rc · $took" --sticky)
  fi
  "$notify" $args </dev/null >/dev/null 2>&1 &!
  return 0
}

add-zsh-hook preexec _quacko_preexec
precmd_functions=(_quacko_precmd ${precmd_functions:#_quacko_precmd})
