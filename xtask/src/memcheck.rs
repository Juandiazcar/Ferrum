//! `cargo xtask memcheck --daemon-only` (ADR 0022, `docs/12` WP-06): the daemon side of the
//! reference scenario of `docs/03` §1 against the daemon allowance of `docs/03` §2.
//!
//! It builds a release `ferrumd` and runs the scenario test `ferrum-daemon/tests/memcheck.rs`
//! against it (`FERRUM_TEST_FERRUMD`): ten sessions of 10 000 retained lines of the reference mix
//! at 200x55, 10 s idle, the daemon's memory read with the OS metric (PSS, `phys_footprint`,
//! Private Working Set). The test holds the gate and prints the `MEMCHECK` line with the number. The
//! full scenario with the UI (`memcheck` without `--daemon-only`) arrives with WP-08.

use std::path::PathBuf;

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;

use crate::out;

/// The scenario test, run alone and with its output shown.
const TEST_ARGS: &[&str] = &[
    "test",
    "--locked",
    "-p",
    "ferrum-daemon",
    "--test",
    "memcheck",
    "--",
    "--ignored",
    "--exact",
    "daemon_memory_in_the_reference_scenario",
    "--nocapture",
    "--test-threads",
    "1",
];

/// Runs the daemon-only scenario; `Ok(true)` when it is within the allowance.
pub(crate) fn run_daemon_only() -> Result<bool> {
    let metadata = MetadataCommand::new().no_deps().exec().context("cargo metadata failed")?;
    let target = PathBuf::from(metadata.target_directory.as_std_path());
    let build =
        ["cargo", "build", "--release", "--locked", "-p", "ferrum-daemon", "--bin", "ferrumd"];
    if !crate::tools::run(&build, &[])? {
        out::error("xtask memcheck: the release ferrumd did not build");
        return Ok(false);
    }
    let daemon = target.join("release").join(format!("ferrumd{}", std::env::consts::EXE_SUFFIX));
    let daemon = daemon.to_str().context("the target directory is not valid UTF-8")?;
    out::info(&format!("xtask memcheck --daemon-only: {daemon}"));
    let argv: Vec<&str> = std::iter::once("cargo").chain(TEST_ARGS.iter().copied()).collect();
    let passed = crate::tools::run(&argv, &[("FERRUM_TEST_FERRUMD", daemon)])?;
    if !passed {
        out::error("xtask memcheck --daemon-only: FAILED (see the MEMCHECK line above)");
    }
    Ok(passed)
}
