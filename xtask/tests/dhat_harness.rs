//! Every integration test that installs `dhat` as the global allocator must be a
//! `harness = false` binary: `dhat` counts every thread, and the libtest harness allocates on its
//! main thread while the test thread measures, which made zero-allocation gates flaky.

#![allow(clippy::expect_used, reason = "test code: a failed expectation is a failed test")]

use std::fs;
use std::path::Path;

#[test]
fn dhat_tests_run_without_the_libtest_harness() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates");
    let mut checked = 0;
    let mut offenders = Vec::new();
    for krate in
        fs::read_dir(&crates).expect("crates dir").map(|entry| entry.expect("entry").path())
    {
        let Ok(tests) = fs::read_dir(krate.join("tests")) else { continue };
        let manifest = fs::read_to_string(krate.join("Cargo.toml")).expect("Cargo.toml");
        let manifest: toml::Table = manifest.parse().expect("valid Cargo.toml");
        let targets =
            manifest.get("test").and_then(toml::Value::as_array).cloned().unwrap_or_default();
        for file in tests.map(|entry| entry.expect("entry").path()) {
            let is_dhat = file.extension().is_some_and(|ext| ext == "rs")
                && fs::read_to_string(&file).expect("test source").contains("dhat::Alloc");
            if !is_dhat {
                continue;
            }
            let stem = file.file_stem().and_then(|stem| stem.to_str()).expect("UTF-8 name");
            let without_harness = targets.iter().any(|target| {
                target.get("name").and_then(toml::Value::as_str) == Some(stem)
                    && target.get("harness").and_then(toml::Value::as_bool) == Some(false)
            });
            if !without_harness {
                offenders.push(file.display().to_string());
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no dhat test found under {}", crates.display());
    assert!(offenders.is_empty(), "dhat tests need `[[test]] harness = false`: {offenders:?}");
}
