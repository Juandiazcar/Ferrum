# Ferrum shell integration for zsh (FR-SESS-005): OSC 7 reports the working directory and OSC 133
# marks prompt (A), input (B), output (C) and the exit status (D). Ferrum points ZDOTDIR at this
# file's directory only so that zsh reads it first. It restores the user's ZDOTDIR at once, so
# every other startup file, system and user, runs exactly as without Ferrum (history, for one,
# stays in the user's HISTFILE), then loads the user's .zshenv. It never edits the user's files.

if [[ -n ${FERRUM_USER_ZDOTDIR+x} ]]; then
    ZDOTDIR=$FERRUM_USER_ZDOTDIR
    unset FERRUM_USER_ZDOTDIR
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR:-$HOME}/.zshenv ]] && source "${ZDOTDIR:-$HOME}/.zshenv"

if [[ -o interactive ]]; then
    # Percent-encodes every byte outside the URL-safe set (LC_ALL=C makes each byte a character).
    __ferrum_osc7() {
        emulate -L zsh
        setopt extended_glob
        local LC_ALL=C
        print -rn -- $'\e]7;file://'"${HOST}${PWD//(#m)[^A-Za-z0-9\/._~-]/%${(l:2::0:)$(( [##16] #MATCH ))}}"$'\a'
    }

    __ferrum_prompted=""

    __ferrum_precmd() {
        local exit_status=$?
        if [[ -n $__ferrum_prompted ]]; then
            print -rn -- $'\e]133;D;'"${exit_status}"$'\a'
        fi
        __ferrum_prompted=1
        __ferrum_osc7
        # Prompt themes may rewrite PS1 every time.
        if [[ $PS1 != *'133;B'* ]]; then
            PS1=$'%{\e]133;A\a%}'"$PS1"$'%{\e]133;B\a%}'
        fi
    }

    __ferrum_preexec() {
        print -rn -- $'\e]133;C\a'
    }

    # Installed at the first prompt, after the user's startup files added their own hooks:
    # first in the list, so `$?` is still the status of the last command.
    __ferrum_install() {
        precmd_functions=(__ferrum_precmd ${precmd_functions:#__ferrum_install})
        preexec_functions+=(__ferrum_preexec)
        unfunction __ferrum_install
        __ferrum_precmd
    }
    precmd_functions+=(__ferrum_install)
fi
