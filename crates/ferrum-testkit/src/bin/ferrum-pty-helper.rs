//! `ferrum-pty-helper`: the portable child process of the `PtyBackend` contract suite
//! (`docs/08` §2: helper binaries instead of shell tricks). One subcommand per behavior:
//!
//! | Command | Behavior |
//! |---|---|
//! | `repl [n]` | prints `LINE0`..`LINE<n-1>` and `END`, then answers every input line `x` with `OUT:x` |
//! | `exit <code>` | exits with `code` |
//! | `cwd` | prints `CWD:<current directory>` and exits |
//! | `env <name>...` | prints `<name>=<value>` or `<name>!` (unset) for each name, then `END` |
//! | `lines <n>` | prints `LINE0`..`LINE<n-1>` and `END`, then exits |
//! | `flood <bytes>` | prints numbered lines until `bytes` are written (`0`: forever), then `FLOOD-END` |
//! | `size` | prints `SIZE <rows> <cols>` at start and after every input line |
//! | `sleep` | stays silent until it is ended |
//! | `lock <path>` | holds an exclusive lock on `path`, prints `LOCKED`, stays alive |
//! | `spawn-lock <path>` | starts `lock <path>` as a grandchild, then stays alive |
//! | `spawn-lock-group <path>` | the same, with the grandchild in a process group of its own |

#![forbid(unsafe_code)]

use std::io::{BufRead, Write};

use ferrum_testkit::pty_contract::{FLOOD_FILLER, FLOOD_LINE_BYTES};

/// Exit code of a helper whose command failed or was unknown (distinct from the codes the
/// contract suite asks for: 0, 3, 255, 259).
const FAILURE_EXIT_CODE: i32 = 101;

/// Windows `CREATE_NEW_PROCESS_GROUP` process creation flag.
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let code = match run(&words) {
        Ok(()) => 0,
        Err(_) => FAILURE_EXIT_CODE,
    };
    std::process::exit(code);
}

fn run(words: &[&str]) -> std::io::Result<()> {
    match words {
        ["repl"] => repl(0),
        ["repl", lines] => repl(number(lines)?),
        ["exit", code] => std::process::exit(i32::try_from(number(code)?).unwrap_or(i32::MAX)),
        ["cwd"] => say(&format!("CWD:{}", std::env::current_dir()?.display())),
        ["env", names @ ..] => env(names),
        ["lines", lines] => print_lines(number(lines)?),
        ["flood", bytes] => flood(number(bytes)?),
        ["size"] => size(),
        ["sleep"] => park(),
        ["lock", path] => lock(path),
        ["spawn-lock", path] => spawn_lock(path, Group::Shared),
        ["spawn-lock-group", path] => spawn_lock(path, Group::Own),
        _ => Err(std::io::Error::other("unknown helper command")),
    }
}

fn number(text: &str) -> std::io::Result<u64> {
    text.parse().map_err(std::io::Error::other)
}

fn say(line: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

fn print_lines(count: u64) -> std::io::Result<()> {
    for index in 0..count {
        say(&format!("LINE{index}"))?;
    }
    say("END")
}

fn repl(count: u64) -> std::io::Result<()> {
    print_lines(count)?;
    for line in std::io::stdin().lock().lines() {
        say(&format!("OUT:{}", line?.trim_end()))?;
    }
    Ok(())
}

fn env(names: &[&str]) -> std::io::Result<()> {
    for name in names {
        match std::env::var_os(name) {
            Some(value) => say(&format!("{name}={}", value.to_string_lossy()))?,
            None => say(&format!("{name}!"))?,
        }
    }
    say("END")
}

fn flood(bytes: u64) -> std::io::Result<()> {
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let filler = "x".repeat(FLOOD_FILLER);
    let mut seq = 0u64;
    while bytes == 0 || seq * FLOOD_LINE_BYTES < bytes {
        // `F` + 8 digits + space + filler + newline = FLOOD_LINE_BYTES.
        writeln!(out, "F{seq:08} {filler}")?;
        seq += 1;
    }
    writeln!(out, "FLOOD-END {seq}")?;
    out.flush()
}

fn size() -> std::io::Result<()> {
    let report = || match terminal_size::terminal_size() {
        Some((width, height)) => say(&format!("SIZE {} {}", height.0, width.0)),
        None => say("SIZE unknown"),
    };
    report()?;
    for line in std::io::stdin().lock().lines() {
        line?;
        report()?;
    }
    Ok(())
}

fn park() -> std::io::Result<()> {
    loop {
        std::thread::park();
    }
}

fn lock(path: &str) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    file.lock()?;
    say("LOCKED")?;
    park()
}

/// Which process group the grandchild of `spawn-lock` runs in.
#[derive(Clone, Copy)]
enum Group {
    Shared,
    Own,
}

fn spawn_lock(path: &str, group: Group) -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args(["lock", path]);
    if matches!(group, Group::Own) {
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            CREATE_NEW_PROCESS_GROUP,
        );
    }
    let _grandchild = command.spawn()?;
    park()
}
