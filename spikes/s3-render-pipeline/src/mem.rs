//! Per-process memory. The spike contains no `unsafe`, so only Linux can be read in-process
//! (PSS from `/proc/self/smaps_rollup`, the `docs/03` §1 metric). On Windows and macOS the app
//! prints `RESULT phase ... pid=N` and waits; `measure-windows.ps1` (Private Working Set via the
//! `Working Set - Private` performance counter) and `measure-macos.sh` (`footprint`) sample it.

use std::time::Duration;

/// Linux PSS/RSS in bytes plus a breakdown; `None` where in-process sampling is not possible.
#[cfg(target_os = "linux")]
fn sample() -> Option<(u64, u64, String)> {
    let text = std::fs::read_to_string("/proc/self/smaps_rollup").ok()?;
    let get = |key: &str| -> u64 {
        text.lines()
            .find_map(|l| {
                l.strip_prefix(key)?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()
            })
            .map_or(0, |kb| kb * 1024)
    };
    let detail = format!(
        "pss_anon={} pss_file={} pss_shmem={}",
        get("Pss_Anon:"),
        get("Pss_File:"),
        get("Pss_Shmem:")
    );
    Some((get("Pss:"), get("Rss:"), detail))
}

#[cfg(not(target_os = "linux"))]
fn sample() -> Option<(u64, u64, String)> {
    None
}

/// Bytes as MiB for display.
#[allow(clippy::cast_precision_loss)]
// INVARIANT: a process footprint is far below 2^52 bytes, so the `f64` conversion is exact.
fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

/// Announce a memory phase. Linux samples PSS in-process; elsewhere the external sampler reads the
/// `pid` from the phase line, so wait for its acknowledgement (or pause when run by hand).
pub(crate) fn report(label: &str) {
    out!("RESULT phase label={label} pid={}", std::process::id());
    match sample() {
        Some((pss, rss, detail)) => out!(
            "RESULT mem phase={label} primary_bytes={pss} primary_mb={:.2} rss_bytes={rss} {detail}",
            mib(pss)
        ),
        None if std::env::var_os("S3_ACK").is_some() => {
            // The sampler writes one line to our stdin when it has finished.
            std::io::stdin().read_line(&mut String::new()).ok();
        }
        None => std::thread::sleep(Duration::from_millis(1500)),
    }
}
