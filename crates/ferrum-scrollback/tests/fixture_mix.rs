//! The product fixture generator (`fixtures/output/`) produces the reference mix of `docs/03` §1,
//! deterministically, and its rows are valid scrollback input.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

#[path = "../../../fixtures/output/output.rs"]
mod fixture;

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use fixture::{Fixture, Profile, REFERENCE_COLS};

const LOGICAL_LINES: usize = 12_000;
const SEEDS: [u64; 3] = [1, 2, 3];

fn percent(part: usize, whole: usize) -> f64 {
    f64::from(u32::try_from(part).unwrap_or(u32::MAX)) * 100.0
        / f64::from(u32::try_from(whole).unwrap_or(u32::MAX))
}

#[test]
fn the_achieved_mix_matches_the_reference_scenario() {
    for seed in SEEDS {
        let (_, stats) = Fixture::rows(seed, Profile::Mixed, REFERENCE_COLS, LOGICAL_LINES);
        let colored = percent(stats.colored, stats.lines);
        let non_ascii = percent(stats.non_ascii, stats.lines);
        let long = percent(stats.long, stats.lines);
        let mean_cols = percent(stats.columns, stats.lines) / 100.0;
        let mean_runs = percent(stats.colored_runs, stats.colored) / 100.0;
        assert!((33.0..37.0).contains(&colored), "seed {seed}: colored {colored:.2} %");
        assert!((3.9..6.1).contains(&non_ascii), "seed {seed}: non-ASCII {non_ascii:.2} %");
        assert!((1.2..2.8).contains(&long), "seed {seed}: long {long:.2} %");
        assert!((66.0..74.0).contains(&mean_cols), "seed {seed}: mean columns {mean_cols:.2}");
        assert!(
            (3.9..4.1).contains(&mean_runs),
            "seed {seed}: runs per colored line {mean_runs:.2}"
        );
        assert!(stats.rows >= 10_000, "enough rows to retain 10 000 after wrapping");
    }
}

#[test]
fn generation_is_deterministic_per_seed_and_differs_between_seeds() {
    let (first, _) = Fixture::rows(7, Profile::Mixed, REFERENCE_COLS, 500);
    let (again, _) = Fixture::rows(7, Profile::Mixed, REFERENCE_COLS, 500);
    let (other, _) = Fixture::rows(8, Profile::Mixed, REFERENCE_COLS, 500);
    assert_eq!(first, again);
    assert_ne!(first, other);
}

#[test]
fn rows_fit_the_width_and_are_accepted_by_the_scrollback() {
    for profile in [Profile::Mixed, Profile::Entropy] {
        let (rows, _) = Fixture::rows(1, profile, REFERENCE_COLS, 2_000);
        let mut store = Scrollback::new(ScrollbackConfig::default());
        for row in &rows {
            assert!(row.text.chars().count() <= REFERENCE_COLS);
            store.push_line(&row.as_line()).expect("fixture rows are valid input");
        }
        assert_eq!(store.len(), rows.len());
    }
}
