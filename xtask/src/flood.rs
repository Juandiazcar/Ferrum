//! `cargo xtask flood [--against <rev>]`: the flood-isolation gate of NFR-PERF-04 against a
//! release `ferrumd` (`ferrum-daemon/tests/flood.rs`), with the comparison of `docs/08` §8.
//!
//! The test itself holds the absolute limit (echo p99 at most 10 ms, NFR-PERF-04). With
//! `--against`, the same test is also run against the `ferrumd` of `rev`, built in a git worktree
//! on this machine, in [`PAIRS`] pairs of short runs, base and head back to back in alternating
//! order (ABBA). Each pair gives the ratio of the head's echo p50 to the base's; a shared runner
//! drifts for both runs of a pair alike, so the ratio cancels it. The gate is the median of the
//! ratios: at most [`MAX_SLOWDOWN`] (maintainer decision Q-34: the p50 is compared, the p99 only
//! has its absolute limit). Too few valid pairs fail; a revision without a daemon has nothing to
//! compare with.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;

use crate::worktree::Worktree;
use crate::{out, tools};

/// Pairs of runs (base, head): enough that the median of their ratios is not moved by a few
/// disturbed pairs, while 32 short runs of about 10 s keep the job near 10 minutes.
const PAIRS: usize = 16;
/// Echo round trips per run: short runs, so the two runs of a pair see the same runner state;
/// 250 still give a p50 within a few percent between runs of the same daemon.
const SAMPLES: &str = "250";
/// Fewest valid pairs (both runs reported a p50) for a verdict: three quarters of them.
const MIN_VALID_PAIRS: usize = PAIRS * 3 / 4;
/// `docs/08` §8: at most 10 % slower than the previous `main`.
const MAX_SLOWDOWN: f64 = 1.10;
/// The flood test, run alone with its output shown.
const TEST_ARGS: &[&str] = &[
    "test",
    "--locked",
    "-p",
    "ferrum-daemon",
    "--test",
    "flood",
    "--",
    "--exact",
    "a_flooding_session_does_not_delay_the_other_nineteen",
    "--nocapture",
];

/// Runs the gate; `Ok(true)` when it passes.
pub(crate) fn run(against: Option<&str>) -> Result<bool> {
    let metadata = MetadataCommand::new().no_deps().exec().context("cargo metadata failed")?;
    let root = PathBuf::from(metadata.workspace_root.as_std_path());
    let target = PathBuf::from(metadata.target_directory.as_std_path());
    let current = build_daemon(&root.join("Cargo.toml"), &target)?
        .context("the release ferrumd of this commit did not build")?;
    let Some(rev) = against else {
        return Ok(measure(&current, &target)?.passed);
    };
    let scratch = target.join("flood-previous");
    let tree = Worktree::add(&root, scratch.join("src"), rev)?;
    let Some(previous) = build_daemon(&tree.path.join("Cargo.toml"), &scratch.join("target"))?
    else {
        out::info(&format!("flood: {rev} has no ferrumd; nothing to compare with"));
        return Ok(measure(&current, &target)?.passed);
    };
    compare(rev, &previous, &current, &target)
}

/// `PAIRS` pairs in ABBA order; the median of the head/base p50 ratios.
fn compare(rev: &str, previous: &Path, current: &Path, target: &Path) -> Result<bool> {
    let mut ratios = Vec::with_capacity(PAIRS);
    for pair in 0..PAIRS {
        let order = if pair.is_multiple_of(2) { [true, false] } else { [false, true] };
        let (mut base, mut head) = (None, None);
        for is_previous in order {
            let (daemon, name) =
                if is_previous { (previous, rev) } else { (current, "this commit") };
            out::info(&format!("flood: pair {}/{PAIRS}: the daemon of {name}", pair + 1));
            let run = measure(daemon, target)?;
            if is_previous {
                // The previous daemon may miss the absolute limit; its p50 still counts.
                base = run.p50;
            } else if !run.passed {
                return Ok(false);
            } else {
                head = run.p50;
            }
        }
        if let (Some(base), Some(head)) = (base, head) {
            let ratio = head / base;
            out::info(&format!(
                "flood: pair {}: p50 {base:.0} / {head:.0} ns, ratio {ratio:.3}",
                pair + 1
            ));
            ratios.push(ratio);
        }
    }
    Ok(verdict(rev, &mut ratios))
}

/// Fails when the median ratio is above `MAX_SLOWDOWN`, or with too few valid pairs.
fn verdict(rev: &str, ratios: &mut [f64]) -> bool {
    if ratios.len() < MIN_VALID_PAIRS {
        out::error(&format!(
            "xtask flood: only {} of {PAIRS} pairs measured both daemons; nothing was compared",
            ratios.len()
        ));
        return false;
    }
    ratios.sort_by(f64::total_cmp);
    let middle = ratios.len() / 2;
    let median = match (ratios.get(middle), middle.checked_sub(1).and_then(|i| ratios.get(i))) {
        (Some(&high), Some(&low)) if ratios.len().is_multiple_of(2) => f64::midpoint(low, high),
        (Some(&value), _) => value,
        _ => f64::INFINITY,
    };
    let percent = (MAX_SLOWDOWN - 1.0) * 100.0;
    out::info(&format!(
        "flood: median p50 ratio this commit / {rev}: {median:.3} over {} pairs",
        ratios.len()
    ));
    if median > MAX_SLOWDOWN {
        out::error(&format!("xtask flood: the echo p50 is more than {percent:.0} % above {rev}"));
        return false;
    }
    true
}

/// Builds the release `ferrumd` of the workspace `manifest`; `None` when it has none.
fn build_daemon(manifest: &Path, target: &Path) -> Result<Option<PathBuf>> {
    let has_daemon = manifest.parent().is_some_and(|dir| dir.join("crates/ferrum-daemon").is_dir());
    if !has_daemon {
        return Ok(None);
    }
    let (manifest, target_arg) = (manifest.to_string_lossy(), target.to_string_lossy());
    let build = [
        "cargo",
        "build",
        "--release",
        "--locked",
        "--manifest-path",
        &manifest,
        "--target-dir",
        &target_arg,
        "-p",
        "ferrum-daemon",
        "--bin",
        "ferrumd",
    ];
    anyhow::ensure!(tools::run(&build, &[])?, "a release ferrumd did not build");
    let exe = format!("ferrumd{}", std::env::consts::EXE_SUFFIX);
    Ok(Some(target.join("release").join(exe)))
}

/// One run of the flood test.
struct Run {
    /// The test passed (its absolute p99 limit included).
    passed: bool,
    /// Its echo p50 in ns, when it wrote its report.
    p50: Option<f64>,
}

/// Runs the flood test against `daemon` (the test holds the absolute p99 limit).
fn measure(daemon: &Path, target: &Path) -> Result<Run> {
    let report = target.join("flood-report.txt");
    // A report left by an earlier run must not be read as this one's.
    let _stale = std::fs::remove_file(&report);
    let daemon = daemon.to_str().context("the target directory is not valid UTF-8")?;
    let report_arg = report.to_str().context("the target directory is not valid UTF-8")?;
    let argv: Vec<&str> = std::iter::once("cargo").chain(TEST_ARGS.iter().copied()).collect();
    let env = [
        ("FERRUM_TEST_FERRUMD", daemon),
        ("FERRUM_FLOOD_REPORT", report_arg),
        ("FERRUM_FLOOD_SAMPLES", SAMPLES),
    ];
    let passed = tools::run(&argv, &env)?;
    if !passed {
        out::error("xtask flood: the flood test failed (see its output above)");
    }
    let p50 = std::fs::read_to_string(&report).ok().and_then(|text| {
        let line = text.lines().find_map(|line| line.strip_prefix("p50_ns "))?;
        line.trim().parse().ok()
    });
    Ok(Run { passed, p50 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratios(values: &[f64]) -> Vec<f64> {
        let mut all = vec![1.0; MIN_VALID_PAIRS];
        all.splice(..values.len(), values.iter().copied());
        all
    }

    #[test]
    fn the_median_of_the_paired_ratios_decides() {
        // A few disturbed pairs do not move the median.
        let mut noisy = ratios(&[1.4, 1.3, 0.7]);
        assert!(verdict("base", &mut noisy));
        let mut slower = vec![1.15; MIN_VALID_PAIRS];
        assert!(!verdict("base", &mut slower), "a consistent 15 % slowdown fails");
        let mut too_few = vec![1.0; MIN_VALID_PAIRS - 1];
        assert!(!verdict("base", &mut too_few), "too few valid pairs fail");
    }
}
