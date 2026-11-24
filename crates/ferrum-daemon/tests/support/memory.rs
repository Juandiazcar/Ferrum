//! The memory of another process with the metric of `docs/03` §1, read without `unsafe`: Linux
//! `Pss` from `/proc/<pid>/smaps_rollup`, macOS `phys_footprint` as `footprint` prints it (the
//! number Activity Monitor shows), Windows "Working Set - Private" through the performance
//! counters (what Task Manager shows).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unreachable_pub,
    reason = "test support"
)]

/// A measured value.
pub struct Measured {
    pub bytes: u64,
    pub metric: &'static str,
}

/// Bytes in a KiB (`smaps_rollup` prints kB).
const KIB: u64 = 1_024;

#[cfg(target_os = "linux")]
pub fn process_memory(pid: u32) -> Measured {
    let text = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap();
    let line = text.lines().find(|line| line.starts_with("Pss:")).expect("no Pss line");
    let kib: u64 = line.trim_start_matches("Pss:").trim_end_matches("kB").trim().parse().unwrap();
    Measured { bytes: kib * KIB, metric: "PSS" }
}

#[cfg(target_os = "macos")]
pub fn process_memory(pid: u32) -> Measured {
    let output = std::process::Command::new("footprint")
        .args(["-f", "bytes", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    // "ferrumd [123]: 64-bit    Footprint: 12582912 B (16384 bytes per page)"
    let rest = text.split("Footprint:").nth(1).expect("no Footprint line");
    let mut words = rest.split_whitespace();
    let bytes: u64 = words.next().unwrap().parse().unwrap();
    assert_eq!(words.next(), Some("B"), "footprint -f bytes prints bytes");
    Measured { bytes, metric: "phys_footprint" }
}

#[cfg(windows)]
pub fn process_memory(pid: u32) -> Measured {
    let script = format!(
        "(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter 'IDProcess={pid}').WorkingSetPrivate"
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    Measured { bytes: text.trim().parse().unwrap(), metric: "Private Working Set" }
}
