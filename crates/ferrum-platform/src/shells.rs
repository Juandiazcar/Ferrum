//! The default shell of the user (`docs/11` §1 and §4), used when no shell profile is configured.
//!
//! Unix (`docs/11` §1): the login shell from `$SHELL`, then the user's entry in the user database
//! (what `getent passwd` reads on Linux and `dscl` on macOS), then the first installed shell
//! listed in `/etc/shells`. Windows (`docs/11` §4): PowerShell 7 (`pwsh.exe`) when it is on the
//! `PATH`, else Windows PowerShell (`powershell.exe`), else `%COMSPEC%` (`cmd.exe`).

use std::ffi::OsString;

#[cfg(unix)]
mod passwd;

/// List of installed shells (`docs/11` §1).
#[cfg(unix)]
const SHELLS_FILE: &str = "/etc/shells";
/// What every POSIX system provides when nothing else names a shell.
#[cfg(unix)]
const POSIX_SHELL: &str = "/bin/sh";

/// The program of the default shell (a path or a bare name the PTY backend looks up).
#[cfg(unix)]
pub fn default_shell() -> OsString {
    std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .or_else(passwd::login_shell)
        .or_else(|| first_installed(&std::fs::read_to_string(SHELLS_FILE).unwrap_or_default()))
        .unwrap_or_else(|| OsString::from(POSIX_SHELL))
}

/// The first shell of an `/etc/shells` text that exists as a file.
#[cfg(unix)]
fn first_installed(list: &str) -> Option<OsString> {
    list.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .find(|line| std::path::Path::new(line).is_file())
        .map(OsString::from)
}

/// The program of the default shell (a path or a bare name the PTY backend looks up).
#[cfg(windows)]
pub fn default_shell() -> OsString {
    ["pwsh.exe", "powershell.exe"]
        .into_iter()
        .find(|name| on_path(name))
        .map(OsString::from)
        .or_else(|| std::env::var_os("COMSPEC"))
        .unwrap_or_else(|| OsString::from("cmd.exe"))
}

#[cfg(windows)]
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn there_is_always_a_shell() {
        assert!(!super::default_shell().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_first_installed_shell_of_the_list_is_taken() {
        let list = "# comment\n\n/no/such/shell\n/bin/sh\n/bin/zsh\n";
        assert_eq!(super::first_installed(list), Some("/bin/sh".into()));
        assert!(super::passwd::login_shell().is_some(), "the test user has a login shell");
    }
}
