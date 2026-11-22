//! Experiment 2: exit codes, kill, reader EOF, fast-exit output race and descendants.

use crate::bytes::contains;
use crate::out::say;
use crate::session::{Sess, spawn};
use crate::sysinfo::{helper, kill_pid, pid_alive};
use anyhow::Result;
use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

pub(crate) fn run() -> Result<()> {
    say!("\n=== [2] exit codes ===");
    basic_exits()?;
    killer_result()?;
    fast_exit_race()?;
    grandchild()
}

fn exit3_command() -> (&'static str, CommandBuilder) {
    #[cfg(windows)]
    {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/c", "exit 3"]);
        ("cmd /c exit 3", cmd)
    }
    #[cfg(unix)]
    {
        let mut cmd = CommandBuilder::new("sh");
        cmd.args(["-c", "exit 3"]);
        ("sh -c 'exit 3'", cmd)
    }
}

fn basic_exits() -> Result<()> {
    let (label, cmd) = exit3_command();
    exit_case(label, cmd, None);
    for (label, code) in [
        ("helper exit 0", "0"),
        ("helper exit 255", "255"),
        ("helper exit 259 (STILL_ACTIVE on Windows)", "259"),
    ] {
        exit_case(label, helper(&["exit", code])?, None);
    }
    exit_case(
        "helper sleep 60, kill()",
        helper(&["sleep", "60"])?,
        Some(Duration::from_millis(500)),
    );
    Ok(())
}

fn exit_case(label: &str, cmd: CommandBuilder, kill_after: Option<Duration>) {
    let mut s = match spawn(cmd, 80, 24) {
        Ok(s) => s,
        Err(e) => {
            say!("[2] {label}: SPAWN FAILED {e:#}");
            return;
        }
    };
    let since = if let Some(delay) = kill_after {
        s.pump(delay);
        let t = Instant::now();
        say!("[2] {label}: child.kill() returned {:?}", s.child.kill());
        t
    } else {
        s.t0
    };
    let exited_at = observe_exit(label, &mut s, since, kill_after.is_some());
    observe_eof(label, &mut s, exited_at);
}

fn observe_exit(label: &str, s: &mut Sess, since: Instant, killed: bool) -> Instant {
    let exit = s.wait_exit(Duration::from_secs(10));
    let now = Instant::now();
    match exit {
        Some((_, st)) => say!(
            "[2] {label}: exit observed {:?} after {} -> exit_code={} success={} debug={st:?}",
            now.saturating_duration_since(since),
            if killed { "kill" } else { "spawn" },
            st.exit_code(),
            st.success()
        ),
        None => say!("[2] {label}: NO EXIT within 10 s"),
    }
    now
}

fn observe_eof(label: &str, s: &mut Sess, exited_at: Instant) {
    match s.wait_end(Duration::from_secs(2)) {
        Some(t) => say!(
            "[2] {label}: reader EOF with master open: {:?} after exit",
            t.saturating_duration_since(exited_at)
        ),
        None => say!("[2] {label}: reader EOF with master open: NONE within 2 s"),
    }
    let dropped_at = Instant::now();
    s.close();
    match s.wait_end(Duration::from_secs(3)) {
        Some(t) => say!(
            "[2] {label}: reader EOF after dropping master/writer: {:?}",
            t.saturating_duration_since(dropped_at)
        ),
        None => say!("[2] {label}: reader EOF after dropping master/writer: NONE within 3 s"),
    }
}

/// `clone_killer().kill()` result (source review suspects an inverted check on Windows).
fn killer_result() -> Result<()> {
    let mut s = spawn(helper(&["sleep", "60"])?, 80, 24)?;
    s.pump(Duration::from_millis(500));
    let mut killer = s.child.clone_killer();
    say!("[2] clone_killer().kill() returned {:?}", killer.kill());
    let exit = s.wait_exit(Duration::from_secs(5));
    say!("[2] exit after clone_killer kill: {:?}", exit.map(|(d, st)| (d, st.exit_code())));
    Ok(())
}

enum Arrival {
    WithMasterOpen,
    OnlyAfterClose,
    Lost,
}

fn race_once(i: usize) -> Result<Arrival> {
    let mut s = spawn(helper(&["echo-bye", &i.to_string()])?, 80, 24)?;
    let tag = format!("BYE_{i}");
    s.wait_exit(Duration::from_secs(5));
    s.pump_until(&tag, Duration::from_millis(500));
    if contains(&s.buf, tag.as_bytes()) {
        return Ok(Arrival::WithMasterOpen);
    }
    s.close();
    s.pump_until(&tag, Duration::from_secs(1));
    Ok(if contains(&s.buf, tag.as_bytes()) { Arrival::OnlyAfterClose } else { Arrival::Lost })
}

/// Does the last output of a fast-exiting child always arrive?
fn fast_exit_race() -> Result<()> {
    let runs = 30;
    let (mut open, mut after_close, mut lost) = (0, 0, 0);
    for i in 0..runs {
        match race_once(i)? {
            Arrival::WithMasterOpen => open += 1,
            Arrival::OnlyAfterClose => after_close += 1,
            Arrival::Lost => lost += 1,
        }
    }
    say!(
        "[2] fast-exit output race over {runs} runs: arrived_with_master_open={open} only_after_master_close={after_close} lost={lost}"
    );
    Ok(())
}

fn grandchild_pid(output: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(output);
    text.split("GRANDCHILD ").nth(1)?.split_whitespace().next()?.parse().ok()
}

/// Does killing the child, then dropping the master, also kill a grandchild?
fn grandchild() -> Result<()> {
    let mut s = spawn(helper(&["spawn-sleep"])?, 80, 24)?;
    s.pump_until("GRANDCHILD", Duration::from_secs(5));
    let pid = grandchild_pid(&s.buf);
    s.child.kill().ok(); // the result is what experiment 2 reports elsewhere; here only the effect matters
    s.wait_exit(Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(500));
    let after_kill = pid.map(pid_alive);
    s.close();
    std::thread::sleep(Duration::from_secs(1));
    let after_close = pid.map(pid_alive);
    say!(
        "[2] grandchild pid={pid:?} alive after child.kill()={after_kill:?} alive after master drop (+1 s)={after_close:?}"
    );
    if let Some(p) = pid.filter(|_| after_close == Some(true)) {
        kill_pid(p);
    }
    Ok(())
}
