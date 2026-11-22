# Ferrum

A cross-platform terminal emulator written in Rust: CPU-rendered, fast, light on memory, with a
daemon that keeps sessions alive and a UI that groups sessions by project.

**Status: pre-alpha.** The repository currently holds the specification, the workspace skeleton
(twelve empty crates plus `xtask`) and CI. There is nothing to install or run yet.

- Specification and build order: [`docs/spec/`](docs/spec/README.md) (start with
  [`docs/12`](docs/spec/docs/12-build-order.md) for the work packages).
- Progress and open decisions: [`PROGRESS.md`](PROGRESS.md).
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md). Security reports: [`SECURITY.md`](SECURITY.md).

## Development

```bash
cargo xtask setup        # git hooks + tool check
cargo xtask check        # everything CI runs
cargo xtask check-deps   # crate layering rules
```

`cargo xtask bench`, `memcheck` and `soak` exist but report "not implemented" until the work
packages that make them meaningful land.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
