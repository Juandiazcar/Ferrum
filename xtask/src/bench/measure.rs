//! Running the criterion benches of one checkout of the workspace.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;

use super::{Effort, criterion};
use crate::tools;

/// A checkout to bench: where its manifest is, where it builds, where criterion writes.
pub(crate) struct Workspace {
    pub(crate) manifest: PathBuf,
    pub(crate) target_dir: PathBuf,
    /// `CRITERION_HOME` of this checkout, so two checkouts never share results.
    pub(crate) criterion_home: PathBuf,
}

/// Runs every bench target of `workspace` and returns the median ns per bench id. A checkout
/// without benches gives an empty map.
pub(crate) fn measure(workspace: &Workspace, effort: Effort) -> Result<BTreeMap<String, f64>> {
    if workspace.criterion_home.exists() {
        // Stale results of earlier runs must not be mistaken for this run.
        fs::remove_dir_all(&workspace.criterion_home)?;
    }
    fs::create_dir_all(&workspace.criterion_home)?;
    let metadata =
        MetadataCommand::new().manifest_path(&workspace.manifest).no_deps().exec().with_context(
            || format!("cargo metadata failed for {}", workspace.manifest.display()),
        )?;
    for package in metadata.workspace_packages() {
        for target in package.targets.iter().filter(|target| target.is_bench()) {
            if !run_target(workspace, &package.name, &target.name, effort)? {
                anyhow::bail!("`{}` bench `{}` failed", package.name, target.name);
            }
        }
    }
    criterion::collect(&workspace.criterion_home)
}

fn run_target(workspace: &Workspace, package: &str, target: &str, effort: Effort) -> Result<bool> {
    let manifest = workspace.manifest.to_string_lossy();
    let target_dir = workspace.target_dir.to_string_lossy();
    let mut argv = vec![
        "cargo",
        "bench",
        "--locked",
        "--manifest-path",
        &manifest,
        "--target-dir",
        &target_dir,
        "-p",
        package,
        "--bench",
        target,
        "--",
        "--noplot",
    ];
    if effort == Effort::Quick {
        argv.push("--quick");
    }
    let home = workspace.criterion_home.to_string_lossy();
    tools::run(&argv, &[("CRITERION_HOME", &home)])
}
