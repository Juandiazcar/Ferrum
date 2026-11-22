# fixtures/output

Deterministic, seeded generators of realistic terminal output used by memory measurements, benches
and tests (docs/08 §4).

**Files:** `output.rs` (API: `Fixture::rows(seed, profile, cols, logical_lines)`), `rng.rs`
(splitmix64), `text.rs` (log-like vocabulary, random ASCII, CJK/emoji injection).

**Content (docs/03 §1):** 35 % colored lines of 3 to 5 style runs, about 70 visible columns on
average, 5 % lines with non-ASCII (accented Latin, CJK, emoji), 2 % lines of 300 to 400 columns,
wrapped into rows of 200 columns (`WRAPPED` flag on all but the last row). Profiles: `Mixed` (the
reference fixture) and `Entropy` (random printable ASCII, the compression lower bound).

**Use:** the file is included with `#[path = "../../../fixtures/output/output.rs"] mod fixture;`
by `crates/ferrum-scrollback/{tests,benches,examples}` (it is not a crate, so the dependency
rules of docs/04 §2 are untouched). `tests/fixture_mix.rs` checks the achieved mix and determinism:
the numbers are counted, never assumed. Colors are fixture-local values; `ferrum-vt` defines the
real encoding.

**Filled by:** WP-01 (rows for scrollback); WP-08 adds the byte-stream rendering for `memcheck`.
