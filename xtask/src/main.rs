//! `cargo xtask`: repository automation (docs/04 §1, docs/05 §3).
//!
//! `memcheck --daemon-only` measures the daemon (WP-06); the full `memcheck` and `soak` need later
//! work packages and are stubs: they print what is missing and exit non-zero, never fake numbers.

mod bench;
mod check;
mod deps;
mod flood;
mod import_refs;
mod memcheck;
mod out;
mod setup;
mod tools;
mod worktree;

use std::path::PathBuf;
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
    /// Run the criterion benches and gate them (docs/03 §6, docs/08 §8): against
    /// `benches/baselines/{os}.json` by default, or against the previous commit with `--against`.
    Bench {
        /// Criterion quick mode: fewer samples, same comparison rules.
        #[arg(long)]
        quick: bool,
        /// Write the baseline of this OS instead of comparing (a deliberate, reviewed act).
        #[arg(long)]
        save_baseline: bool,
        /// Bench this git revision and the current commit on this machine and compare them (CI).
        #[arg(long, value_name = "REV", conflicts_with = "save_baseline")]
        against: Option<String>,
    },
    /// Convert the reference tests of `alacritty_terminal` (`tests/ref`, Apache-2.0) into the
    /// compact fixtures of `fixtures/vt/alacritty/` (docs/08 section 4).
    ImportAlacrittyRefs {
        /// The `tests/ref` directory of an `alacritty_terminal` source tree.
        #[arg(long, value_name = "DIR")]
        from: PathBuf,
        /// Where the fixtures are written.
        #[arg(long, value_name = "DIR", default_value = "fixtures/vt/alacritty")]
        to: PathBuf,
    },
    /// Reference-scenario memory measurement: `--daemon-only` measures `ferrumd` (WP-06); the
    /// full scenario with the UI arrives with WP-08.
    Memcheck {
        /// Measure the daemon alone against its allowance (`docs/03` §2).
        #[arg(long)]
        daemon_only: bool,
    },
    /// The flood-isolation gate (NFR-PERF-04) against a release `ferrumd`, and with `--against`
    /// the comparison with the daemon of an earlier revision on this machine (`docs/08` §8).
    Flood {
        /// Also run the gate against the daemon of this git revision and compare (CI).
        #[arg(long, value_name = "REV")]
        against: Option<String>,
    },
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
        Command::ImportAlacrittyRefs { from, to } => import_refs::run(&from, &to),
        Command::Bench { quick, save_baseline, against } => bench::run(
            if quick { bench::Effort::Quick } else { bench::Effort::Full },
            &match (save_baseline, against) {
                (true, _) => bench::Mode::SaveBaseline,
                (false, Some(rev)) => bench::Mode::Against(rev),
                (false, None) => bench::Mode::Compare,
            },
        ),
        Command::Memcheck { daemon_only: true } => memcheck::run_daemon_only(),
        Command::Memcheck { daemon_only: false } => {
            return not_implemented("memcheck", "WP-08", "no UI to measure yet; use --daemon-only");
        }
        Command::Flood { against } => flood::run(against.as_deref()),
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
