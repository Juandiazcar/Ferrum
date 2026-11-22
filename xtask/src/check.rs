//! `cargo xtask check`: the local mirror of CI (docs/06 §3).

use anyhow::Result;

use crate::tools::{self, TOOLS};

struct Step {
    name: &'static str,
    argv: &'static [&'static str],
    envs: &'static [(&'static str, &'static str)],
    /// Part of the `--fast` subset used by the pre-push hook.
    fast: bool,
}

const STEPS: &[Step] = &[
    Step { name: "fmt", argv: &["cargo", "fmt", "--all", "--check"], envs: &[], fast: true },
    Step {
        name: "clippy",
        argv: &[
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        envs: &[],
        fast: true,
    },
    Step {
        name: "nextest",
        argv: &["cargo", "nextest", "run", "--workspace", "--locked"],
        envs: &[],
        fast: false,
    },
    Step {
        name: "doctests",
        argv: &["cargo", "test", "--workspace", "--doc", "--locked"],
        envs: &[],
        fast: false,
    },
    Step { name: "deny", argv: &["cargo", "deny", "check"], envs: &[], fast: false },
    Step { name: "check-deps", argv: &["cargo", "xtask", "check-deps"], envs: &[], fast: true },
    Step { name: "typos", argv: &["typos"], envs: &[], fast: true },
    Step { name: "machete", argv: &["cargo-machete"], envs: &[], fast: false },
    Step {
        name: "docs",
        argv: &["cargo", "doc", "--workspace", "--no-deps", "--locked"],
        envs: &[("RUSTDOCFLAGS", "-D warnings")],
        fast: false,
    },
    Step {
        name: "features",
        argv: &[
            "cargo",
            "hack",
            "check",
            "--workspace",
            "--each-feature",
            "--no-dev-deps",
            "--locked",
        ],
        envs: &[],
        fast: false,
    },
];

pub(crate) fn run(fast: bool) -> Result<bool> {
    let missing: Vec<_> = TOOLS.iter().filter(|tool| !tools::is_installed(tool)).collect();
    if !missing.is_empty() {
        for tool in &missing {
            eprintln!("missing tool {}: {}", tool.name, tool.install);
        }
        eprintln!("Missing tools are an error, never a skip. See `cargo xtask setup`.");
        return Ok(false);
    }
    let mut failed = Vec::new();
    for step in STEPS.iter().filter(|step| step.fast || !fast) {
        println!("==> {}: {}", step.name, step.argv.join(" "));
        if !tools::run(step.argv, step.envs)? {
            failed.push(step.name);
        }
    }
    if failed.is_empty() {
        println!("xtask check: all steps passed");
    } else {
        eprintln!("xtask check: FAILED steps: {}", failed.join(", "));
    }
    Ok(failed.is_empty())
}
