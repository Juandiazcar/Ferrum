# assets/shell-integration

Shell integration scripts for bash, zsh, fish and PowerShell (FR-SESS-005): OSC 7 reports the
working directory, OSC 133 marks prompt, input, output and exit status. They never edit the user's
startup files.

| File | Shell | How `ferrum-pty` loads it |
|---|---|---|
| `ferrum.bash` | bash | `--rcfile`; it loads `~/.bashrc` (or the login files) itself |
| `zshenv.zsh` | zsh | written as `.zshenv` in a private `ZDOTDIR`; it restores the user's `ZDOTDIR` first |
| `ferrum.fish` | fish | `--init-command`, after the user's configuration |
| `ferrum.ps1` | PowerShell 5.1 and 7 | inline with `-NoExit -EncodedCommand` (never a script file) |

**Added by:** WP-05 (`ferrum_pty::shell_integration`, which compiles them in). WP-12 builds prompt
navigation on top of the marks.
