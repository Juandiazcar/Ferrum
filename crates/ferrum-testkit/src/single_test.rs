//! The `main` of a `harness = false` test binary that holds one test and runs it on the main
//! thread.
//!
//! The `dhat` tests need it: `dhat` counts the allocations of every thread, and under the libtest
//! harness the test runs on a spawned thread while the main thread is still allocating (its
//! bookkeeping for the spawned test, then the channel it blocks on), so a zero-allocation window
//! that opens early races with it.

/// Runs `test` as the binary's only test, named `name`. Speaks the part of the libtest command
/// line that `cargo test` and `cargo nextest` use: `--list` (`name: test`) and `--ignored`
/// (the test is not ignored, so nothing is listed or run). Does nothing under Miri, which does
/// not support `dhat`. A failure panics, which fails the process.
///
/// shortcut: name filters and other libtest flags are ignored (the test always runs); parse them
/// if a binary ever holds more than one test.
#[allow(clippy::print_stdout, reason = "the libtest protocol is stdout")]
pub fn single_test(name: &str, test: fn()) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|arg| arg == flag);
    if has("--ignored") {
        return;
    }
    if has("--list") {
        println!("{name}: test");
        return;
    }
    if cfg!(miri) {
        return;
    }
    test();
    println!("test {name} ... ok");
}
