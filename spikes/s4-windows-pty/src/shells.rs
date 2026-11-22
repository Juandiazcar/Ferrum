//! The interactive shells the experiments drive, per OS.

use portable_pty::CommandBuilder;

pub(crate) struct Shell {
    pub(crate) name: &'static str,
    pub(crate) cmd: CommandBuilder,
    /// Line typed into the shell.
    pub(crate) line: &'static str,
    /// Text the *result* contains but the typed line does not (so echo is not mistaken for output).
    pub(crate) marker: &'static str,
}

fn shell(name: &'static str, program: &str, args: &[&str], line: &'static str) -> Shell {
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    Shell { name, cmd, line, marker: "FERRUM_S4_ECHO" }
}

#[cfg(windows)]
pub(crate) fn shells() -> Vec<Shell> {
    let ps_line = "echo ('FERRUM_S4_'+'ECHO')\r";
    let ps_args = ["-NoLogo", "-NoProfile"];
    vec![
        shell("cmd.exe", "cmd.exe", &[], "echo FERRUM_S4_^ECHO\r"),
        shell("powershell.exe", "powershell.exe", &ps_args, ps_line),
        shell("pwsh", "pwsh", &ps_args, ps_line),
    ]
}

#[cfg(unix)]
pub(crate) fn shells() -> Vec<Shell> {
    let line = "echo FERRUM_S4_'ECHO'\r";
    vec![
        shell("sh -i", "sh", &["-i"], line),
        shell("bash -i", "bash", &["--norc", "--noprofile", "-i"], line),
    ]
}

/// A plain interactive shell for the resize experiments.
pub(crate) fn idle_shell() -> CommandBuilder {
    #[cfg(windows)]
    {
        CommandBuilder::new("cmd.exe")
    }
    #[cfg(unix)]
    {
        let mut cmd = CommandBuilder::new("sh");
        cmd.arg("-i");
        cmd
    }
}
