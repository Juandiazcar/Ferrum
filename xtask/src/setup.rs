//! `cargo xtask setup`: git hooks, toolchain and tool check (docs/06 §3).

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::out;
use crate::tools::{self, TOOLS};

const HOOKS_DIR: &str = ".githooks";
const TOOLCHAIN_FILE: &str = "rust-toolchain.toml";

#[derive(Deserialize)]
struct ToolchainFile {
    toolchain: Pin,
}

/// The `[toolchain]` table of `rust-toolchain.toml`.
#[derive(Deserialize)]
struct Pin {
    channel: String,
    #[serde(default)]
    components: Vec<String>,
}

/// Components whose installed name differs from the name listed in `rust-toolchain.toml`.
fn installed_name(component: &str) -> &str {
    if component == "llvm-tools-preview" { "llvm-tools" } else { component }
}

/// Compares the active toolchain with the pin; returns one message per mismatch.
fn toolchain_problems(pin: &Pin, rustc_version: &str, installed_components: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let active = rustc_version.split_whitespace().nth(1).unwrap_or_default();
    if active != pin.channel {
        problems.push(format!(
            "active rustc is `{active}` but {TOOLCHAIN_FILE} pins `{}` (run `rustup show` in the repository root)",
            pin.channel
        ));
    }
    for component in &pin.components {
        let name = installed_name(component);
        let prefix = format!("{name}-");
        let present = installed_components.lines().any(|line| line.trim().starts_with(&prefix));
        if !present {
            problems.push(format!(
                "component `{component}` is not installed (rustup component add {component})"
            ));
        }
    }
    problems
}

fn check_toolchain() -> Result<bool> {
    let text = std::fs::read_to_string(TOOLCHAIN_FILE)
        .with_context(|| format!("cannot read {TOOLCHAIN_FILE}"))?;
    let pin = toml::from_str::<ToolchainFile>(&text)
        .with_context(|| format!("invalid {TOOLCHAIN_FILE}"))?
        .toolchain;
    let version = tools::capture(&["rustc", "--version"])?;
    let components = tools::capture(&["rustup", "component", "list", "--installed"])?;
    let problems = toolchain_problems(&pin, &version, &components);
    for problem in &problems {
        out::error(&format!("toolchain MISMATCH: {problem}"));
    }
    if problems.is_empty() {
        out::info(&format!("toolchain ok: {}", version.trim()));
    }
    Ok(problems.is_empty())
}

pub(crate) fn run() -> Result<bool> {
    let hooks_ok = tools::run(&["git", "config", "core.hooksPath", HOOKS_DIR], &[])?;
    if hooks_ok {
        out::info(&format!("git hooks: core.hooksPath={HOOKS_DIR}"));
    }
    let toolchain_ok = check_toolchain()?;
    let mut all_tools = true;
    for tool in TOOLS {
        if tools::is_installed(tool) {
            out::info(&format!("tool ok:      {}", tool.name));
        } else {
            all_tools = false;
            out::info(&format!("tool MISSING: {}  ->  {}", tool.name, tool.install));
        }
    }
    Ok(hooks_ok && toolchain_ok && all_tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin() -> Pin {
        Pin {
            channel: "1.100.0".into(),
            components: vec!["clippy".into(), "llvm-tools-preview".into()],
        }
    }

    const COMPONENTS: &str = "clippy-x86_64-unknown-linux-gnu\nllvm-tools-x86_64-unknown-linux-gnu\nrustfmt-x86_64-unknown-linux-gnu\n";

    #[test]
    fn matching_toolchain_has_no_problems() {
        assert_eq!(
            toolchain_problems(&pin(), "rustc 1.100.0 (abc 2026-11-09)", COMPONENTS),
            Vec::<String>::new()
        );
    }

    #[test]
    fn reports_version_mismatch() {
        let problems = toolchain_problems(&pin(), "rustc 1.96.0 (abc 2026-05-25)", COMPONENTS);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("`1.96.0`") && problems[0].contains("`1.100.0`"));
    }

    #[test]
    fn reports_missing_component() {
        let problems = toolchain_problems(
            &pin(),
            "rustc 1.100.0 (abc 2026-11-09)",
            "clippy-x86_64-unknown-linux-gnu\n",
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("llvm-tools-preview"));
    }
}
