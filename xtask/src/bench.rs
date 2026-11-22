//! `cargo xtask bench`: runs the criterion benches of the workspace and gates them (`docs/03` §6,
//! `docs/08` §8). Three modes:
//!
//! - default: compare the medians with the committed baseline of this OS
//!   (`benches/baselines/{os}.json`, for a maintainer's machine);
//! - `--save-baseline`: write that baseline (a deliberate, reviewed act);
//! - `--against <rev>`: bench `rev` and the current commit on this runner in this job and compare
//!   them (CI, where shared runners make numbers from other machines meaningless).
//!
//! All modes fail on a bench more than 10 % slower, on a bench that disappeared, and on an absolute
//! ceiling (2x the budget target). Crates without benches have nothing to run; nothing is faked
//! for them.

mod against;
mod baseline;
mod criterion;
mod measure;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use baseline::{Baseline, Finding, SCHEMA};
use cargo_metadata::MetadataCommand;
use measure::{Workspace, measure};

use crate::out;

/// How long criterion measures.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effort {
    /// Full sampling (CI, baselines).
    Full,
    /// `--quick`: criterion's quick mode for a fast local signal; same comparison rules.
    Quick,
}

/// What to do with the results.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Compare with the committed baseline of this OS.
    Compare,
    /// Write the baseline of this OS.
    SaveBaseline,
    /// Compare with the same benches at this git revision, run in this job.
    Against(String),
}

pub(crate) fn run(effort: Effort, mode: &Mode) -> Result<bool> {
    let metadata = MetadataCommand::new().no_deps().exec().context("cargo metadata failed")?;
    let root = PathBuf::from(metadata.workspace_root.as_std_path());
    let target_dir = PathBuf::from(metadata.target_directory.as_std_path());
    let current = Workspace {
        manifest: root.join("Cargo.toml"),
        criterion_home: target_dir.join("criterion"),
        target_dir,
    };
    if let Mode::Against(rev) = mode {
        return against::run(&current, &root, rev, effort);
    }
    let results = measure(&current, effort)?;
    if results.is_empty() {
        out::error("xtask bench: no criterion results were produced");
        return Ok(false);
    }
    let path =
        root.join("benches").join("baselines").join(format!("{}.json", std::env::consts::OS));
    match mode {
        Mode::SaveBaseline => save(&path, results),
        _ => compare(&path, &results),
    }
}

fn save(path: &PathBuf, current: BTreeMap<String, f64>) -> Result<bool> {
    let baseline =
        Baseline { schema: SCHEMA, os: std::env::consts::OS.to_owned(), median_ns: current };
    fs::write(path, serde_json::to_string_pretty(&baseline)? + "\n")?;
    out::info(&format!("bench: wrote {} ({} benches)", path.display(), baseline.median_ns.len()));
    Ok(true)
}

fn compare(path: &PathBuf, current: &BTreeMap<String, f64>) -> Result<bool> {
    let Ok(text) = fs::read_to_string(path) else {
        out::error(&format!("xtask bench: no baseline for this OS at {}", path.display()));
        out::error(
            "Create it deliberately with `cargo xtask bench --save-baseline` and commit it.",
        );
        return Ok(false);
    };
    let baseline: Baseline = serde_json::from_str(&text).context("baseline file is invalid")?;
    anyhow::ensure!(
        baseline.schema == SCHEMA,
        "baseline schema {} is not supported",
        baseline.schema
    );
    let findings = baseline::compare(&baseline, current);
    Ok(report(&findings, &baseline.median_ns, current, &path.display().to_string()))
}

/// Prints every bench next to its reference and the findings; true when there are none.
fn report(
    findings: &[Finding],
    reference: &BTreeMap<String, f64>,
    current: &BTreeMap<String, f64>,
    against: &str,
) -> bool {
    for (id, now) in current {
        match reference.get(id) {
            Some(before) => {
                let change = (now / before - 1.0) * 100.0;
                out::info(&format!(
                    "bench: {id}: {now:.0} ns ({before:.0} ns reference, {change:+.1} %)"
                ));
            }
            None => out::info(&format!("bench: {id}: {now:.0} ns")),
        }
    }
    for finding in findings {
        out::error(&format!("bench FAILED: {finding}"));
    }
    if findings.is_empty() {
        out::info(&format!("bench: {} benches within +10 % of {against}", current.len()));
    }
    findings.is_empty()
}
