//! `cargo xtask bench --against <rev>`: the CI gate of `docs/08` §8. Shared runners are too noisy
//! to compare with numbers measured elsewhere, so the previous commit is benched on the same
//! runner in the same job, interleaved with the current one, and the two are compared.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use super::baseline::{self, MAX_SLOWDOWN};
use super::measure::{Workspace, measure};
use super::{Effort, report};
use crate::out;
use crate::worktree::Worktree;

/// Rounds of (previous, current). Each side keeps its best median per bench, which cancels a slow
/// moment of the runner without touching the 10 % limit.
const ROUNDS: usize = 2;

pub(crate) fn run(current: &Workspace, root: &Path, rev: &str, effort: Effort) -> Result<bool> {
    let scratch = current.target_dir.join("bench-previous");
    let tree = Worktree::add(root, scratch.join("src"), rev)?;
    let previous = Workspace {
        manifest: tree.path.join("Cargo.toml"),
        target_dir: scratch.join("target"),
        criterion_home: scratch.join("criterion"),
    };
    let (mut before, mut after) = (BTreeMap::new(), BTreeMap::new());
    for round in 1..=ROUNDS {
        out::info(&format!("bench: round {round}/{ROUNDS}: previous commit {rev}"));
        keep_best(&mut before, measure(&previous, effort)?);
        out::info(&format!("bench: round {round}/{ROUNDS}: current commit"));
        keep_best(&mut after, measure(current, effort)?);
    }
    if after.is_empty() {
        out::error("xtask bench: the current commit produced no criterion results");
        return Ok(false);
    }
    let findings = baseline::compare_runs(&before, &after);
    for id in after.keys().filter(|id| !before.contains_key(*id)) {
        out::info(&format!("bench: {id}: new in this commit, nothing to compare with"));
    }
    Ok(report(
        &findings,
        &before,
        &after,
        &format!("{rev} (limit +{:.0} %)", (MAX_SLOWDOWN - 1.0) * 100.0),
    ))
}

fn keep_best(best: &mut BTreeMap<String, f64>, round: BTreeMap<String, f64>) {
    for (id, ns) in round {
        best.entry(id).and_modify(|kept| *kept = kept.min(ns)).or_insert(ns);
    }
}
