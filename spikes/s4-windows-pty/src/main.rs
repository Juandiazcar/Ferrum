//! Spike S4 (docs/12): exercises `portable-pty` (`ConPTY` on Windows, Unix pty elsewhere). THROWAWAY;
//! results and decision in docs/adr/0026.
//!
//! Usage: `s4-windows-pty [all|1..8]`. Experiments print `key = value` lines; the other
//! subcommands (`flood`, `sleep`, `exit`, `env`, `spawn-sleep`, `echo-bye`) are helper children.

mod bytes;
mod exp1_echo;
mod exp2_exit;
mod exp3_resize;
mod exp4_large;
mod exp5_startup;
mod exp6_io;
mod exp7_env;
mod exp8_close;
mod helpers;
mod out;
mod session;
mod shells;
mod stats;
mod sysinfo;

use anyhow::Result;
use out::say;
use std::panic::catch_unwind;

type Experiment = (&'static str, fn() -> Result<()>);

const EXPERIMENTS: [Experiment; 8] = [
    ("1", exp1_echo::run),
    ("2", exp2_exit::run),
    ("3", exp3_resize::run),
    ("4", exp4_large::run),
    ("5", exp5_startup::run),
    ("6", exp6_io::run),
    ("7", exp7_env::run),
    ("8", exp8_close::run),
];

/// Runs the selected experiments; returns how many failed (error or caught panic).
fn run_selected(which: &str) -> usize {
    let mut failures = 0;
    for (name, run) in EXPERIMENTS {
        if which != "all" && which != name {
            continue;
        }
        match catch_unwind(run) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                say!("[{name}] ERROR: {e:#}");
                failures += 1;
            }
            Err(_) => {
                say!("[{name}] PANIC inside experiment (caught)");
                failures += 1;
            }
        }
    }
    failures
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if helpers::run(&args)? {
        return Ok(());
    }
    say!(
        "s4 harness: os={} arch={} portable-pty=0.9.0",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let failures = run_selected(args.first().map_or("all", String::as_str));
    // Leaked reader threads blocked in `read` must not keep the process alive.
    std::process::exit(i32::from(failures > 0));
}
