//! Running external tools and the list of tools `setup`/`check` require.

use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// A cargo subcommand or binary that `cargo xtask check` needs, with how to install it.
pub(crate) struct Tool {
    pub(crate) name: &'static str,
    /// Argv used to probe that the tool works.
    pub(crate) probe: &'static [&'static str],
    /// Pinned install command, versions from docs/07 §4.
    pub(crate) install: &'static str,
}

pub(crate) const TOOLS: &[Tool] = &[
    Tool {
        name: "cargo-nextest",
        probe: &["cargo", "nextest", "--version"],
        install: "cargo install --locked cargo-nextest@0.9.152",
    },
    Tool {
        name: "cargo-deny",
        probe: &["cargo", "deny", "--version"],
        install: "cargo install --locked cargo-deny@0.22.0",
    },
    Tool {
        name: "typos-cli",
        probe: &["typos", "--version"],
        install: "cargo install --locked typos-cli@1.52.2",
    },
    Tool {
        name: "cargo-machete",
        probe: &["cargo-machete", "--version"],
        install: "cargo install --locked cargo-machete@0.10.1",
    },
    Tool {
        name: "cargo-hack",
        probe: &["cargo", "hack", "--version"],
        install: "cargo install --locked cargo-hack@0.6.46",
    },
];

/// Whether the tool can be run at all.
pub(crate) fn is_installed(tool: &Tool) -> bool {
    let Some((program, args)) = tool.probe.split_first() else {
        return false;
    };
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Runs `argv` with inherited output; `Ok(true)` when it exited with status 0.
pub(crate) fn run(argv: &[&str], envs: &[(&str, &str)]) -> Result<bool> {
    let (command, rest) = argv.split_first().context("empty command")?;
    let status = Command::new(command)
        .args(rest)
        .envs(envs.iter().copied())
        .status()
        .with_context(|| format!("failed to start `{command}`"))?;
    Ok(status.success())
}
