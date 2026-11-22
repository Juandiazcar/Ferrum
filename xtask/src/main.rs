//! `cargo xtask`: repository automation (docs/04 §1, docs/05 §3).
//!
//! Subcommands that need later work packages are stubs: they print what is missing and exit
//! non-zero, never fake numbers.

mod check;
mod deps;
mod out;
mod setup;
mod tools;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Ferrum repository automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Activate git hooks (core.hooksPath=.githooks) and check the required tools.
    Setup,
    /// Run everything CI runs: fmt, clippy, tests, doctests, deny, check-deps, typos, docs.
    Check {
        /// Fast subset used by the pre-push hook: fmt, clippy, check-deps, typos.
        #[arg(long)]
        fast: bool,
    },
    /// Validate the crate dependency graph against xtask/deps-rules.toml (docs/04 §2).
    CheckDeps,
    /// Criterion benches against committed baselines (not implemented yet).
    Bench(StubArgs),
    /// Reference-scenario memory measurement (not implemented yet).
    Memcheck(StubArgs),
    /// Long-running leak check (not implemented yet).
    Soak(StubArgs),
}

/// Flags the real commands will take later (`--quick`, `--ci`, ...); ignored by the stubs.
#[derive(clap::Args)]
struct StubArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

/// Exit code of stub commands: distinct from 1 (a failed check) so logs are unambiguous.
const EXIT_NOT_IMPLEMENTED: u8 = 2;

fn not_implemented(command: &str, work_package: &str, reason: &str) -> ExitCode {
    out::error(&format!("xtask {command}: not implemented until {work_package} ({reason})."));
    out::error("No measurement was taken and none is reported.");
    ExitCode::from(EXIT_NOT_IMPLEMENTED)
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Setup => setup::run(),
        Command::Check { fast } => {
            check::run(if fast { check::Scope::Fast } else { check::Scope::Full })
        }
        Command::CheckDeps => deps::run(),
        Command::Bench(_) => {
            return not_implemented("bench", "WP-01/WP-02", "no benches exist before those crates");
        }
        Command::Memcheck(_) => {
            return not_implemented("memcheck", "WP-08", "no binaries to measure yet");
        }
        Command::Soak(_) => {
            return not_implemented("soak", "WP-08", "no daemon or UI to soak yet");
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            out::error(&format!("xtask: {error:#}"));
            ExitCode::FAILURE
        }
    }
}
