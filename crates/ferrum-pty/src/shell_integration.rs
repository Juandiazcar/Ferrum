//! Shell integration bootstrap (FR-SESS-005, `docs/11` §1): small scripts that make bash, zsh,
//! fish and `PowerShell` report the working directory (OSC 7) and mark prompts and command
//! boundaries (OSC 133), injected at spawn without ever editing the user's startup files.
//!
//! The daemon writes the scripts once into a private directory with
//! [`write_bootstrap_files`] and calls [`inject`] on each spawn whose profile has integration on
//! (it is opt-out per profile, `docs/09` §settings). Injection per shell (`docs/11` §1):
//!
//! | Shell | How the script is loaded | The user's files |
//! |---|---|---|
//! | bash | `--rcfile <dir>/ferrum.bash` | the script loads `~/.bashrc`, or the login files when the profile asked for a login shell |
//! | zsh | `ZDOTDIR=<dir>/zsh` (its `.zshenv`) | the script restores the user's `ZDOTDIR` at once and loads their `.zshenv`; zsh then reads every other file as usual |
//! | fish | `--init-command 'source <dir>/ferrum.fish'` | fish loads them itself, before the command |
//! | `PowerShell` | `-NoExit -EncodedCommand <script>` (inline, so no execution policy applies) | `PowerShell` loads the profile itself, before the command |
//!
//! The scripts live in `assets/shell-integration/` (`docs/04` §1) and are compiled in.
//!
//! A spawn with other arguments (a command, a script) is left untouched: integration only applies
//! to an interactive shell.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::{EnvOp, PtyError, SpawnSpec};

/// A shell that has an integration script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shell {
    /// GNU bash.
    Bash,
    /// zsh.
    Zsh,
    /// fish.
    Fish,
    /// `PowerShell` 7 (`pwsh`) or Windows `PowerShell` 5.1 (`powershell`).
    PowerShell,
}

impl Shell {
    /// Recognizes the shell from the program's file name (`bash`, `/bin/zsh`, `pwsh.exe`, ...).
    pub fn detect(program: &OsStr) -> Option<Self> {
        let stem = Path::new(program).file_stem()?.to_str()?.to_ascii_lowercase();
        match stem.as_str() {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            "pwsh" | "powershell" => Some(Self::PowerShell),
            _ => None,
        }
    }
}

const BASH: &str = "ferrum.bash";
const FISH: &str = "ferrum.fish";
const ZSH_DIR: &str = "zsh";

/// Every bootstrap file: path under the directory and contents.
const FILES: [(&str, &str); 3] = [
    (BASH, include_str!("../../../assets/shell-integration/ferrum.bash")),
    (FISH, include_str!("../../../assets/shell-integration/ferrum.fish")),
    ("zsh/.zshenv", include_str!("../../../assets/shell-integration/zshenv.zsh")),
];

/// The `PowerShell` script, passed inline rather than written to a file.
const POWERSHELL: &str = include_str!("../../../assets/shell-integration/ferrum.ps1");

/// Writes (or rewrites) every bootstrap file under `dir`, which should be the daemon's private
/// runtime directory. On Unix the directories are made owner-only (`0700`) and every file
/// owner-read-write (`0600`), also when it already existed with other permissions, so no other
/// user can read or replace what a shell will source (`docs/10` §2). On Windows they inherit the
/// directory's access list.
///
/// # Errors
/// [`PtyError::Io`] when a directory or file cannot be created or written.
pub fn write_bootstrap_files(dir: &Path) -> Result<(), PtyError> {
    private_dir(dir)?;
    private_dir(&dir.join(ZSH_DIR))?;
    for (name, contents) in FILES {
        write_private(&dir.join(name), contents.as_bytes())?;
    }
    Ok(())
}

#[cfg(unix)]
fn private_dir(dir: &Path) -> Result<(), PtyError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let error = |source| PtyError::io("create the shell integration directory", source);
    std::fs::DirBuilder::new().recursive(true).mode(OWNER_ONLY_DIR).create(dir).map_err(error)?;
    // An existing directory may have been created with looser permissions.
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(OWNER_ONLY_DIR)).map_err(error)
}

#[cfg(windows)]
fn private_dir(dir: &Path) -> Result<(), PtyError> {
    std::fs::create_dir_all(dir)
        .map_err(|source| PtyError::io("create the shell integration directory", source))
}

/// Owner read, write and search (`docs/10` §2: private runtime files).
#[cfg(unix)]
const OWNER_ONLY_DIR: u32 = 0o700;
/// Owner read and write.
#[cfg(unix)]
const OWNER_ONLY_FILE: u32 = 0o600;

fn write_private(path: &Path, contents: &[u8]) -> Result<(), PtyError> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, OWNER_ONLY_FILE);
    let error = |source| PtyError::io("write a shell integration file", source);
    let mut file = options.open(path).map_err(error)?;
    // `mode` applies only when the file is created; an existing file may be looser.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = std::fs::Permissions::from_mode(OWNER_ONLY_FILE);
        file.set_permissions(private).map_err(error)?;
    }
    file.write_all(contents).map_err(error)
}

/// Changes `spec` so the shell loads its bootstrap file from `dir` (written by
/// [`write_bootstrap_files`]). Returns the shell, or `None` when nothing was changed: the program
/// is not a known shell, the spawn has its own arguments (only login flags are accepted, and
/// `-NoLogo` for `PowerShell`), or, for fish only, `dir` is not valid Unicode (the path goes into a
/// fish command string).
pub fn inject(spec: &mut SpawnSpec, dir: &Path) -> Option<Shell> {
    let shell = Shell::detect(&spec.program)?;
    let login = match login_flag(shell, &spec.args)? {
        Flags::None => false,
        Flags::Login => true,
    };
    match shell {
        Shell::Bash => {
            spec.args = vec!["--rcfile".into(), dir.join(BASH).into_os_string()];
            if login {
                set(spec, "FERRUM_BASH_LOGIN", "1".into());
            }
        }
        Shell::Zsh => {
            if let Some(user_dir) = inherited(spec, "ZDOTDIR") {
                set(spec, "FERRUM_USER_ZDOTDIR", user_dir);
            }
            set(spec, "ZDOTDIR", dir.join(ZSH_DIR).into_os_string());
        }
        Shell::Fish => {
            let script = fish_quote(dir.join(FISH).to_str()?);
            spec.args.extend(["--init-command".into(), format!("source {script}").into()]);
        }
        Shell::PowerShell => {
            let script = encoded_command(POWERSHELL);
            spec.args.extend(["-NoExit".into(), "-EncodedCommand".into(), script.into()]);
        }
    }
    Some(shell)
}

/// What the spawn's own arguments ask for, when integration may be added to them.
enum Flags {
    None,
    Login,
}

fn login_flag(shell: Shell, args: &[OsString]) -> Option<Flags> {
    let is = |arg: &OsString, accepted: &[&str]| {
        arg.to_str().is_some_and(|arg| accepted.iter().any(|flag| arg.eq_ignore_ascii_case(flag)))
    };
    match (shell, args) {
        (_, []) => Some(Flags::None),
        (Shell::PowerShell, args) if args.iter().all(|arg| is(arg, &["-NoLogo", "-Login"])) => {
            Some(Flags::None)
        }
        (Shell::Bash | Shell::Zsh | Shell::Fish, [arg]) if is(arg, &["-l", "--login"]) => {
            Some(Flags::Login)
        }
        _ => None,
    }
}

/// The value `name` will have in the child: the spawn's own change if any, else inherited.
fn inherited(spec: &SpawnSpec, name: &str) -> Option<OsString> {
    match spec.env.iter().rev().find(|(key, _)| key == name) {
        Some((_, EnvOp::Set(value))) => Some(value.clone()),
        Some((_, EnvOp::Remove)) => None,
        None => std::env::var_os(name),
    }
}

fn set(spec: &mut SpawnSpec, name: &str, value: OsString) {
    spec.env.push((name.into(), EnvOp::Set(value)));
}

/// A fish single-quoted string: only `\` and `'` are escaped.
fn fish_quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The standard Base64 alphabet (RFC 4648 §4), which `-EncodedCommand` expects.
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `script` as `PowerShell`'s `-EncodedCommand` wants it: Base64 of its UTF-16LE bytes.
fn encoded_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut text = String::with_capacity(bytes.len().div_ceil(GROUP_BYTES) * DIGITS_PER_GROUP);
    for group in bytes.chunks(GROUP_BYTES) {
        let [a, b, c] = [0, 1, 2].map(|index| group.get(index).copied().unwrap_or(0));
        let bits = u32::from_be_bytes([0, a, b, c]);
        // A short final group is padded with `=` for the digits it does not fill.
        for (digit, shift) in DIGIT_SHIFTS.into_iter().enumerate() {
            let symbol = BASE64.get(usize::try_from((bits >> shift) & DIGIT_MASK).unwrap_or(0));
            let padding = digit > group.len();
            text.push(if padding { '=' } else { symbol.map_or('=', |&byte| char::from(byte)) });
        }
    }
    text
}

/// Base64 encodes groups of 3 bytes (24 bits) ...
const GROUP_BYTES: usize = 3;
/// ... as 4 digits of 6 bits each ...
const DIGITS_PER_GROUP: usize = 4;
/// ... taken from the high bits down ...
const DIGIT_SHIFTS: [u32; DIGITS_PER_GROUP] = [18, 12, 6, 0];
/// ... 6 bits at a time.
const DIGIT_MASK: u32 = 0x3F;

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;
    use std::path::PathBuf;

    use super::*;
    use crate::PtySize;

    fn spec(program: &str, args: &[&str]) -> SpawnSpec {
        SpawnSpec {
            program: program.into(),
            args: args.iter().map(OsString::from).collect(),
            cwd: PathBuf::from("."),
            env: Vec::new(),
            size: PtySize { cols: NonZeroU16::MIN, rows: NonZeroU16::MIN },
        }
    }

    fn env_value(spec: &SpawnSpec, name: &str) -> Option<EnvOp> {
        spec.env.iter().rev().find(|(key, _)| key == name).map(|(_, op)| op.clone())
    }

    #[test]
    fn detects_shells_by_file_name() {
        assert_eq!(Shell::detect("/usr/bin/bash".as_ref()), Some(Shell::Bash));
        assert_eq!(Shell::detect("zsh".as_ref()), Some(Shell::Zsh));
        assert_eq!(Shell::detect("fish".as_ref()), Some(Shell::Fish));
        assert_eq!(Shell::detect("PWSH.EXE".as_ref()), Some(Shell::PowerShell));
        assert_eq!(Shell::detect("powershell".as_ref()), Some(Shell::PowerShell));
        assert_eq!(Shell::detect("cmd.exe".as_ref()), None);
        assert_eq!(Shell::detect("bashful".as_ref()), None);
    }

    #[test]
    fn bash_gets_the_rcfile_and_a_login_marker() {
        let dir = Path::new("/run/ferrum");
        let mut login = spec("bash", &["--login"]);
        assert_eq!(inject(&mut login, dir), Some(Shell::Bash));
        assert_eq!(login.args, ["--rcfile".into(), dir.join(BASH).into_os_string()]);
        assert_eq!(env_value(&login, "FERRUM_BASH_LOGIN"), Some(EnvOp::Set("1".into())));
        let mut plain = spec("bash", &[]);
        inject(&mut plain, dir);
        assert_eq!(env_value(&plain, "FERRUM_BASH_LOGIN"), None);
    }

    #[test]
    fn zsh_gets_zdotdir_and_keeps_the_users() {
        let dir = Path::new("/run/ferrum");
        let mut zsh = spec("zsh", &["-l"]);
        zsh.env.push(("ZDOTDIR".into(), EnvOp::Set("/home/u/.config/zsh".into())));
        assert_eq!(inject(&mut zsh, dir), Some(Shell::Zsh));
        assert_eq!(zsh.args, ["-l"]);
        let user = EnvOp::Set("/home/u/.config/zsh".into());
        assert_eq!(env_value(&zsh, "FERRUM_USER_ZDOTDIR"), Some(user));
        assert_eq!(env_value(&zsh, "ZDOTDIR"), Some(EnvOp::Set(dir.join("zsh").into_os_string())));
    }

    #[test]
    fn fish_quotes_the_script_path_and_powershell_gets_the_script_inline() {
        let dir = Path::new("/tmp/it's here");
        let mut fish = spec("fish", &[]);
        inject(&mut fish, dir);
        let fish_path = dir.join(FISH).to_str().map(|path| path.replace('\\', r"\\"));
        let fish_path = fish_path.unwrap_or_default().replace('\'', r"\'");
        assert_eq!(fish.args[1], OsString::from(format!("source '{fish_path}'")));
        let mut pwsh = spec("pwsh", &["-NoLogo"]);
        inject(&mut pwsh, dir);
        assert_eq!(pwsh.args[..3], ["-NoLogo", "-NoExit", "-EncodedCommand"]);
        assert_eq!(pwsh.args[3], OsString::from(encoded_command(POWERSHELL)));
    }

    #[test]
    fn encodes_like_powershell_expects() {
        // `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes("dir"))` is "ZABpAHIA".
        assert_eq!(encoded_command("dir"), "ZABpAHIA");
        assert_eq!(encoded_command("ab"), "YQBiAA==");
        assert_eq!(encoded_command("é"), "6QA=");
    }

    #[test]
    fn commands_and_unknown_programs_are_left_alone() {
        let dir = Path::new("/run/ferrum");
        for (program, args) in [("bash", &["-c", "ls"][..]), ("zsh", &["script.zsh"]), ("vim", &[])]
        {
            let mut untouched = spec(program, args);
            assert_eq!(inject(&mut untouched, dir), None);
            assert_eq!(untouched, spec(program, args));
        }
    }
}
