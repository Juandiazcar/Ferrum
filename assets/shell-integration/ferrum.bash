# Ferrum shell integration for bash (FR-SESS-005): OSC 7 reports the working directory and
# OSC 133 marks prompt (A), input (B), output (C, bash 4.4 and later) and the exit status (D).
# Ferrum starts bash with `--rcfile` pointing here, so this file loads the user's startup files
# itself, exactly as bash would, and never edits them.

if [ -n "${FERRUM_BASH_LOGIN-}" ]; then
    unset FERRUM_BASH_LOGIN
    [ -r /etc/profile ] && . /etc/profile
    for __ferrum_file in ~/.bash_profile ~/.bash_login ~/.profile; do
        if [ -r "$__ferrum_file" ]; then
            . "$__ferrum_file"
            break
        fi
    done
    unset __ferrum_file
else
    [ -r /etc/bash.bashrc ] && . /etc/bash.bashrc
    [ -r ~/.bashrc ] && . ~/.bashrc
fi

# Percent-encodes every byte outside the URL-safe set, so any directory name is reported intact.
__ferrum_osc7() {
    local LC_ALL=C path="" index char code
    for (( index = 0; index < ${#PWD}; index++ )); do
        char=${PWD:index:1}
        case "$char" in
            [a-zA-Z0-9/._~-]) path+="$char" ;;
            *)
                # bash 3.2 reports bytes above 127 as negative numbers: keep the low 8 bits.
                printf -v code '%d' "'$char"
                printf -v char '%%%02X' $(( code & 255 ))
                path+="$char"
                ;;
        esac
    done
    printf '\033]7;file://%s%s\007' "${HOSTNAME-}" "$path"
}

__ferrum_prompted=""

# Runs first in PROMPT_COMMAND, so `$?` is still the status of the last command.
__ferrum_precmd() {
    local status=$?
    if [ -n "$__ferrum_prompted" ]; then
        printf '\033]133;D;%s\007' "$status"
    fi
    __ferrum_prompted=1
    __ferrum_osc7
}

# Runs last in PROMPT_COMMAND: prompt themes may rewrite PS1 every time.
__ferrum_mark_prompt() {
    case "$PS1" in
        *'133;B'*) ;;
        *) PS1='\[\033]133;A\007\]'"$PS1"'\[\033]133;B\007\]' ;;
    esac
}

PROMPT_COMMAND="__ferrum_precmd${PROMPT_COMMAND:+
$PROMPT_COMMAND}
__ferrum_mark_prompt"

if [ "${BASH_VERSINFO[0]}" -gt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -ge 4 ]; }; then
    PS0="${PS0-}"'\033]133;C\007'
fi
