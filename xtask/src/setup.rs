//! `cargo xtask setup`: git hooks and tool check.

use anyhow::Result;

use crate::out;
use crate::tools::{self, TOOLS};

const HOOKS_DIR: &str = ".githooks";

pub(crate) fn run() -> Result<bool> {
    let hooks_ok = tools::run(&["git", "config", "core.hooksPath", HOOKS_DIR], &[])?;
    if hooks_ok {
        out::info(&format!("git hooks: core.hooksPath={HOOKS_DIR}"));
    }
    let mut all_tools = true;
    for tool in TOOLS {
        if tools::is_installed(tool) {
            out::info(&format!("tool ok:      {}", tool.name));
        } else {
            all_tools = false;
            out::info(&format!("tool MISSING: {}  ->  {}", tool.name, tool.install));
        }
    }
    Ok(hooks_ok && all_tools)
}
