//! Reading criterion's results: `target/criterion/<group>/<bench>/new/estimates.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Estimates {
    median: Estimate,
}

#[derive(Deserialize)]
struct Estimate {
    /// Nanoseconds.
    point_estimate: f64,
}

/// Median time in nanoseconds of every bench found under `root`, keyed by `<group>/<bench>`.
pub(crate) fn collect(root: &Path) -> Result<BTreeMap<String, f64>> {
    let mut results = BTreeMap::new();
    walk(root, root, &mut results)?;
    Ok(results)
}

fn walk(root: &Path, dir: &Path, results: &mut BTreeMap<String, f64>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "new") {
            record(root, &path, results)?;
        } else {
            walk(root, &path, results)?;
        }
    }
    Ok(())
}

fn record(root: &Path, new_dir: &Path, results: &mut BTreeMap<String, f64>) -> Result<()> {
    let file = new_dir.join("estimates.json");
    let Ok(text) = fs::read_to_string(&file) else { return Ok(()) };
    let estimates: Estimates = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a criterion estimates file", file.display()))?;
    let bench = new_dir.parent().context("`new` has a parent")?;
    let id = bench
        .strip_prefix(root)?
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    results.insert(id, estimates.median.point_estimate);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_median_of_nested_benches() -> Result<()> {
        let root = std::env::temp_dir().join(format!("xtask-criterion-{}", std::process::id()));
        let new_dir = root.join("group").join("bench").join("new");
        fs::create_dir_all(&new_dir)?;
        fs::write(
            new_dir.join("estimates.json"),
            r#"{"mean":{"point_estimate":9.0},"median":{"point_estimate":42.5}}"#,
        )?;
        let found = collect(&root)?;
        fs::remove_dir_all(&root)?;
        assert_eq!(found.get("group/bench"), Some(&42.5));
        Ok(())
    }
}
