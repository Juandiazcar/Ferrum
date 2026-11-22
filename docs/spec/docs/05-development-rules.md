# 05 — Development Rules

These rules apply to every line of code, whether written by a person or an AI agent. CI enforces what can be enforced; reviewers enforce the rest.

## 1. Clean code, in practice

1. **Names carry intent.** If a comment is needed to explain *what* a name means, rename it.
2. **Small functions with one job.** Soft limit ~40 lines, ≤ 4 parameters (group the rest into a struct), nesting depth ≤ 3. Break any of these only in hot paths where inlining is measured to matter, with a `// perf:` comment pointing to the benchmark.
3. **Comments explain *why*, never *what*.** Required: `// SAFETY:` for every `unsafe`, `// perf:` for deliberately ugly hot-path code, spec/RFC links for protocol behavior (e.g. `// xterm ctlseqs: CSI Ps J`).
4. **No dead code, no commented-out code, no TODO without an issue number** (`// TODO(#123): …`). `todo!()` and `unimplemented!()` are denied by lint.
5. **Make illegal states unrepresentable.** Prefer enums with data over flag soup; newtypes for IDs, units (`Cols(u16)`, `Rows(u16)`, `Px(f32)`); `NonZero*` where zero is invalid.
6. **Immutability by default.** `let` over `let mut`; return new values rather than mutate through many layers; confine mutation to the owner of the data.
7. **Early returns** over deep `if` pyramids; `?` for propagation; `let … else` for guard clauses.
8. **No boolean parameters** that flip behavior; use an enum (`Wrap::Soft`) or two functions.
9. **No magic numbers.** Named `const` with units in the name (`HOT_CHUNK_LINES`, `FRAME_BUDGET_MS`), documented with the reason.
10. **One level of abstraction per function.** Orchestration functions call helpers; helpers do the details.
11. **Consistency beats cleverness.** Follow the surrounding style; use `rustfmt` as law.
12. **Delete before you add.** A change that removes code is preferred when behavior is preserved.

## 2. SOLID, translated to Rust

| Principle | How it applies in Ferrum | Concrete rule |
|---|---|---|
| **S**ingle Responsibility | One reason to change per module/crate | `ferrum-vt` knows nothing about sockets; `ferrum-proto` knows nothing about fonts; a struct that mixes parsing and I/O is split |
| **O**pen/Closed | Extend through traits and enums with `#[non_exhaustive]`, not by editing core logic | Adding a status-bar item = implementing `StatusItem`; adding an OSC code = registering a handler; no `match` on "kind" strings scattered around |
| **L**iskov Substitution | Every trait impl must honor the trait's documented contract, including error and ordering behavior | `PtyBackend` impls (Unix, ConPTY, fake) run the **same shared contract test-suite** in `ferrum-testkit` |
| **I**nterface Segregation | Small, focused traits; no god-trait | `PtyReader`, `PtyWriter`, `PtyResize` rather than one `Pty` with 15 methods; widgets receive only the context slice they need |
| **D**ependency Inversion | High-level policy depends on abstractions, not on OS or I/O details; implementations are injected in the composition root | `ferrum-scrollback` defines `SpillStore`, `ferrum-daemon` defines `Clock`, `ferrum-pty` exposes the `PtyBackend` traits; `ferrumd`'s `main` wires the real implementations; tests wire fakes |

Additional structural principles: **composition over inheritance** (Rust has none; avoid deep trait hierarchies), **command pattern** for UI actions, **state machines as enums** for protocols and session lifecycles, **ports & adapters** at every I/O boundary, **parse, don't validate** at boundaries (convert untrusted bytes to typed values once).

Dynamic dispatch (`dyn Trait`) only at architectural seams where it enables injection or Phase 2 extension; inside engines use generics/enums so the optimizer can inline.

## 3. Error handling

- Libraries: `thiserror` enums, one per crate area, with context-carrying variants; never `anyhow` in libraries; errors implement `std::error::Error + Send + Sync + 'static`.
- Binaries (`ferrum-app`, `ferrumd`, `xtask`): `anyhow` at the top level only, with `.context(...)`.
- **No `unwrap()`, `expect()`, `panic!`, indexing that can panic, or `unreachable!()` in non-test code** without an `#[allow(...)]` that has a `// INVARIANT:` justification. Lints deny `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, and warn on `indexing_slicing` (hot paths may allow it per-module with a bounds proof in a comment).
- Arithmetic on untrusted values uses `checked_*`/`saturating_*`; casts use `try_from` unless provably safe (`#[allow(clippy::cast_possible_truncation)]` with a comment).
- User-facing errors have actionable messages; internal details go to `tracing` logs.
- Never swallow errors silently. If ignoring is correct, say why: `let _ = tx.send(ev); // receiver gone during shutdown`.

## 4. `unsafe` policy

- Default is `#![forbid(unsafe_code)]` in every crate. Exceptions: `ferrum-pty`, `ferrum-platform`, `ferrum-render` (blit kernels only, in a single `unsafe`-allowed module).
- Every `unsafe` block is minimal, has a `// SAFETY:` comment stating the invariants and why they hold, and is covered by a test (Miri where feasible, ASan/UBSan job nightly on Linux).
- Prefer a safe, audited crate over hand-rolled `unsafe`. Prefer safe SIMD via `std::arch` wrappers behind runtime feature detection with a scalar fallback that tests compare against.
- New `unsafe` requires reviewer sign-off and a line in the PR description.

## 5. Concurrency

- Ownership transfer over shared state. Message passing with **bounded** channels. No unbounded queues.
- No `Mutex`/`RwLock` in the parse and paint hot paths. If shared state is unavoidable, document the lock order.
- No blocking calls on the UI main thread (file, socket, DNS, process spawn). Use the IPC thread or a worker.
- No `async` runtime in v1.0 (size and RAM). If Phase 2 networking needs one, it is isolated in its own crate and thread (`docs/13`).
- Every spawned thread: named, stack size set, join handle owned, shutdown path tested.
- Cancellation is cooperative: long jobs check a `CancelToken` at chunk boundaries.

## 6. Performance rules (hot paths)

Hot paths: `vt::parse`, `scrollback::push_line`, `proto::encode_row`, `render::paint_rows`.

1. No heap allocation in steady state (verified by `dhat` tests).
2. No `String` per cell/line; no `format!` in loops; no `clone()` of buffers.
3. Use `memchr` for scanning; process runs of printable ASCII in bulk.
4. Avoid hash maps keyed by strings in hot paths; use interned `u32` IDs.
5. Measure before optimizing: every `// perf:` comment references a benchmark name; unexplained complexity is removed.
6. Keep data cache-friendly: contiguous arrays, small `Copy` structs, `#[repr(C)]` only where layout matters and is tested with `size_of` assertions (`const _: () = assert!(size_of::<Cell>() == 16);`).

## 7. API design

- Newtypes for IDs and units. `#[non_exhaustive]` on public enums/structs that may grow. Builders only for types with ≥ 4 optional fields.
- Take `&str`/`&[T]`/`impl AsRef<Path>`; return owned data only when needed. Accept `impl Into<…>` sparingly.
- Public functions document: purpose, errors, panics (should be "none"), complexity if non-obvious, and have a doc test when practical.
- Breaking a public API inside the workspace is fine; update all call sites in the same commit.
- Types that cross the IPC boundary live only in `ferrum-proto`, are `serde`-derived, and have round-trip + golden-bytes tests.

## 8. Logging and diagnostics

- `tracing` only. No `println!`/`eprintln!`/`dbg!` (denied by lint) except in `xtask` output helpers.
- Levels: `error` (user-impacting), `warn` (recoverable anomaly), `info` (lifecycle, sparse), `debug`/`trace` (developer). Release default is `warn` to a rotating file in `state_dir` (max 1 MB × 3), never to the terminal.
- Never log terminal content, keystrokes, environment values, or paths of user files at `info` or above (privacy).
- Spans around: session lifecycle, resize/reflow, spill, IPC handshake.

## 9. Documentation

- `#![warn(missing_docs)]` on all library crates; CI treats warnings as errors.
- Crate README + module-level `//!` explaining the *model* (how the pieces fit), not repeating signatures.
- Behavior that deviates from xterm or other terminals is documented in `docs/user/compat.md` with a link to the test.
- Architecture-impacting decisions get an ADR (`docs/adr/NNNN-title.md`: context, decision, consequences, status).

## 10. Dependencies

1. **Latest stable at the time of adding**, verified against crates.io (`cargo search`/API), recorded in `docs/07` with the date. Never from memory.
2. Justify each new dependency in the PR: what it replaces, size impact (`cargo bloat`/`cargo tree -e normal`), license (`cargo deny`), maintenance (last release, open issues), `unsafe` footprint, MSRV.
3. `default-features = false` and enable only what is used.
4. Prefer small, focused crates; avoid frameworks.
5. Weekly automated update PRs (Dependabot/Renovate) keep everything current; a major bump is a dedicated PR with benchmarks.
6. Ban list lives in `deny.toml` (e.g., `openssl`, `tokio` in core crates, `time` 0.1, unmaintained crates flagged by RustSec).

## 11. Lint and format configuration (summary; full files in `templates/`)

- `rustfmt`: edition 2024 style, `max_width = 100`, `use_small_heuristics = "Max"`, imports grouped (std / external / crate) via `group_imports = "StdExternalCrate"` where available on the stable channel — if unstable on stable, rely on default ordering and do not use nightly rustfmt.
- Clippy: `all` deny; `pedantic` warn with a short allow-list in the workspace manifest; `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout`, `print_stderr` deny; `indexing_slicing` warn; `missing_const_for_fn` off (noise).
- rustc lints: `missing_docs`, `unreachable_pub`, `unused_qualifications`, `unused_results` off (noise), `let_underscore_drop` warn.
- CI runs with `RUSTFLAGS="-D warnings"`.

## 12. Definition of Done (per change)

- [ ] Code compiles on Linux, macOS, Windows (CI green).
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `cargo xtask check-deps` pass.
- [ ] Tests added/updated: unit + (where behavior crosses crates) integration; bug fixes include a regression test that failed before.
- [ ] No new allocation in hot paths; benchmarks and `memcheck` unchanged or improved (attach numbers if touching hot code).
- [ ] Docs updated (rustdoc, crate README, `docs/user`, ADR if architectural).
- [ ] Requirement IDs referenced in the commit/PR (`Refs: FR-…`).
- [ ] No new `unsafe` without justification; no new dependency without the `docs/07` entry.
- [ ] Commit history follows `docs/06`.

## 13. Review checklist (for reviewers and for the agent's self-review)

1. Does each module still have a single reason to change?
2. Could this be simpler without losing a requirement?
3. What happens on bad input, partial reads, closed sockets, resize to 1×1, 10 000×10 000?
4. Any allocation, lock, or syscall added to a hot path?
5. Are names and docs truthful after this change?
6. Is behavior identical on all three OS, or is the difference documented?
7. Would a new contributor understand this in five minutes?
