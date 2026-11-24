# Ferrum shell integration for fish (FR-SESS-005): OSC 7 reports the working directory and
# OSC 133 marks prompt (A), input (B), output (C) and the exit status (D). Ferrum loads this file
# with `--init-command`, after the user's configuration, and never edits it.

function __ferrum_prompt --on-event fish_prompt
    printf '\e]7;file://%s%s\a' (prompt_hostname) (string escape --style=url -- $PWD)
    printf '\e]133;A\a'
end

function __ferrum_preexec --on-event fish_preexec
    printf '\e]133;C\a'
end

function __ferrum_postexec --on-event fish_postexec
    printf '\e]133;D;%s\a' $status
end

# Marks the end of the prompt (B) after the user's prompt function, whichever it is.
if functions -q fish_prompt
    functions --copy fish_prompt __ferrum_user_prompt
    function fish_prompt
        __ferrum_user_prompt
        printf '\e]133;B\a'
    end
end
