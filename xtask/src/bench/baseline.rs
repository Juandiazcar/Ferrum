//! Baseline file and the comparison rules of `docs/03` §6 (fail if more than 10 % slower).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A bench may be at most this much slower than its baseline (`docs/03` §6: +10 %).
pub(crate) const MAX_SLOWDOWN: f64 = 1.10;

/// Version of the baseline file layout.
pub(crate) const SCHEMA: u32 = 1;

/// Absolute ceilings: 2x the targets of the performance budget (`docs/03` §4 table; `docs/08` §8:
/// "a loose absolute ceiling (2x target) that always fails"). They fail whatever
/// the reference run says. Nanoseconds.
pub(crate) const CEILINGS_NS: &[(&str, f64)] = &[
    // "Scrollback search over 10 000 lines: <= 15 ms", so the ceiling is 30 ms.
    ("scrollback_search/literal_10k_lines", 30_000_000.0),
    ("scrollback_search/literal_insensitive_10k_lines", 30_000_000.0),
    // The daemon's search path (snapshot, screen copy, joined wrapped rows): the same target.
    ("daemon_search/literal_insensitive_10k_lines", 30_000_000.0),
    ("daemon_search/regex_insensitive_10k_lines", 30_000_000.0),
    // NFR-PERF-02 (8 MiB inputs, the product configuration: with the scrollback push on, Q-21):
    // 400 MB/s on plain ASCII is 20.97 ms, so the ceiling is 41.94 ms; 120 MB/s on SGR-heavy
    // output is 69.9 ms, so 139.81 ms. The state machine alone (no history) must stay under the
    // same ceilings.
    ("vt_parse/ascii_8mib", 41_943_040.0),
    ("vt_parse/sgr_heavy_8mib", 139_810_133.0),
    ("vt_parse/ascii_8mib_no_history", 41_943_040.0),
    ("vt_parse/sgr_heavy_8mib_no_history", 139_810_133.0),
    // The worker seals the history the parser retires (Q-21 amendment, ADR 0028), so it must
    // keep up with NFR-PERF-02's 400 MB/s; an encoded row is about as long as its input. The
    // bench seals 857 466 encoded bytes (11 fixture chunks): 2.14 ms at 400 MB/s, so 4.29 ms.
    ("scrollback_seal/fixture_chunks", 4_287_330.0),
    // "Reflow of 10 000 lines on resize <= 40 ms", so the ceiling is 80 ms.
    ("vt_reflow/10k_lines_200_to_100_columns", 80_000_000.0),
    // NFR-PERF-03: full repaint of 200x60 cells <= 2.5 ms CPU, on dense text (every cell a glyph,
    // maintainer decision Q-19) and on the typical mix, so the ceiling is 5 ms.
    ("render_repaint/dense_200x60", 5_000_000.0),
    ("render_repaint/typical_200x60", 5_000_000.0),
];

/// `benches/baselines/{os}.json`: median time per bench id, measured on one runner class.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Baseline {
    pub(crate) schema: u32,
    pub(crate) os: String,
    pub(crate) median_ns: BTreeMap<String, f64>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Finding {
    Slower { id: String, baseline: f64, current: f64 },
    Missing { id: String },
    Unbaselined { id: String },
    OverCeiling { id: String, ceiling: f64, current: f64 },
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slower { id, baseline, current } => {
                let percent = (current / baseline - 1.0) * 100.0;
                write!(
                    f,
                    "{id}: {current:.0} ns vs reference {baseline:.0} ns (+{percent:.1} %, limit +10 %)"
                )
            }
            Self::Missing { id } => {
                write!(f, "{id}: was measured in the reference run but not in this one")
            }
            Self::Unbaselined { id } => {
                write!(f, "{id}: not in the baseline (run `cargo xtask bench --save-baseline`)")
            }
            Self::OverCeiling { id, ceiling, current } => {
                write!(f, "{id}: {current:.0} ns is above the absolute ceiling {ceiling:.0} ns")
            }
        }
    }
}

/// Compares a run with the committed baseline and the absolute ceilings; empty means pass.
/// A bench that has no baseline entry fails: the baseline must be saved deliberately.
pub(crate) fn compare(baseline: &Baseline, current: &BTreeMap<String, f64>) -> Vec<Finding> {
    let mut findings = slower_or_missing(&baseline.median_ns, current);
    for id in current.keys().filter(|id| !baseline.median_ns.contains_key(*id)) {
        findings.push(Finding::Unbaselined { id: id.clone() });
    }
    findings.extend(over_ceiling(current));
    findings
}

/// Compares a run with a run of the previous commit made on the same runner in the same job
/// (`docs/08` §8). A bench the previous commit did not have cannot have regressed and is not a
/// finding; one that disappeared is.
pub(crate) fn compare_runs(
    previous: &BTreeMap<String, f64>,
    current: &BTreeMap<String, f64>,
) -> Vec<Finding> {
    let mut findings = slower_or_missing(previous, current);
    findings.extend(over_ceiling(current));
    findings
}

fn slower_or_missing(
    reference: &BTreeMap<String, f64>,
    current: &BTreeMap<String, f64>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (id, &base) in reference {
        match current.get(id) {
            None => findings.push(Finding::Missing { id: id.clone() }),
            Some(&now) if now > base * MAX_SLOWDOWN => {
                findings.push(Finding::Slower { id: id.clone(), baseline: base, current: now });
            }
            Some(_) => {}
        }
    }
    findings
}

fn over_ceiling(current: &BTreeMap<String, f64>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for &(id, ceiling) in CEILINGS_NS {
        if let Some(&now) = current.get(id).filter(|&&now| now > ceiling) {
            findings.push(Finding::OverCeiling { id: id.to_owned(), ceiling, current: now });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline(entries: &[(&str, f64)]) -> Baseline {
        Baseline {
            schema: SCHEMA,
            os: "test".to_owned(),
            median_ns: entries.iter().map(|&(id, ns)| (id.to_owned(), ns)).collect(),
        }
    }

    fn run(entries: &[(&str, f64)]) -> BTreeMap<String, f64> {
        entries.iter().map(|&(id, ns)| (id.to_owned(), ns)).collect()
    }

    #[test]
    fn ten_percent_is_the_limit() {
        let base = baseline(&[("a", 100.0)]);
        assert!(compare(&base, &run(&[("a", 110.0)])).is_empty(), "exactly +10 % passes");
        assert!(compare(&base, &run(&[("a", 90.0)])).is_empty(), "faster passes");
        let slower = compare(&base, &run(&[("a", 110.5)]));
        assert!(matches!(slower.as_slice(), [Finding::Slower { .. }]));
    }

    #[test]
    fn missing_and_new_benches_fail() {
        let findings = compare(&baseline(&[("a", 1.0)]), &run(&[("b", 1.0)]));
        assert_eq!(
            findings,
            [Finding::Missing { id: "a".to_owned() }, Finding::Unbaselined { id: "b".to_owned() }]
        );
    }

    #[test]
    fn a_run_against_the_previous_commit_allows_new_benches_only() {
        let previous = run(&[("a", 100.0), ("gone", 1.0)]);
        let findings = compare_runs(&previous, &run(&[("a", 105.0), ("new", 5.0)]));
        assert_eq!(findings, [Finding::Missing { id: "gone".to_owned() }]);
        let slower = compare_runs(&previous, &run(&[("a", 120.0), ("gone", 1.0)]));
        assert!(matches!(slower.as_slice(), [Finding::Slower { .. }]));
    }

    #[test]
    fn absolute_ceilings_fail_even_when_the_baseline_agrees() {
        let id = "scrollback_search/literal_10k_lines";
        let slow = 31_000_000.0;
        let findings = compare(&baseline(&[(id, slow)]), &run(&[(id, slow)]));
        assert!(matches!(findings.as_slice(), [Finding::OverCeiling { .. }]));
    }
}
