# 10 — Security and Privacy

A terminal executes arbitrary programs and renders arbitrary bytes from untrusted sources (remote hosts, files, web content piped to the terminal). The threat model reflects that.

## 1. Assets and threats

| Asset | Threat | Mitigation (section) |
|---|---|---|
| User's shell and files | Malicious escape sequences that type into the shell or exfiltrate data | §3 hardening |
| Clipboard | OSC 52 reads/writes by remote programs | §3.4 |
| Local IPC socket | Another local user/process connects to the daemon and injects input | §2 |
| Pasted text | Paste-jacking: hidden control sequences or end-of-paste markers in clipboard text | §3.2 |
| Dropped file paths | Shell injection through crafted file names | §3.5 |
| Update channel | Malicious update | §5 |
| Dependencies | Supply-chain compromise | §6 |
| Phase 2 secrets | API keys leaking via logs/exports/agents | §7 |
| Privacy | Telemetry, content logging | §4 |

## 2. IPC security

- Linux: socket in `$XDG_RUNTIME_DIR/ferrum/` (mode 0700 dir, 0600 socket); fallback `/tmp/ferrum-<uid>/` created with `O_EXCL` + `0700` and ownership verification. macOS: `$TMPDIR` / `~/Library/Application Support/Ferrum/run` with 0700. Windows: named pipe `\\.\pipe\ferrum-<SID-hash>` with a DACL granting only the current user (and SYSTEM).
- On connect, the daemon verifies peer credentials (`SO_PEERCRED`/`LOCAL_PEERCRED` on Unix; pipe client token on Windows) match the daemon's user. Refuse otherwise.
- Handshake includes a random per-daemon nonce stored in a 0600 file; clients must present it. Defense in depth against same-user sandbox escapes is limited, and documented.
- Frame size cap 4 MiB; per-connection rate and memory limits; malformed frames close the connection and are logged (without content).
- Single-instance lock file prevents two daemons for the same user.

## 3. Terminal hardening (tested under AC-SEC-01)

1. **Title/icon reporting** (`CSI 21 t`, `OSC` title queries) is **not implemented**; title set is allowed but sanitized (control chars stripped, length ≤ 1024). Prevents "title injection" attacks.
2. **Bracketed paste**: when enabled by the app, pasted text is wrapped in `ESC[200~ … ESC[201~` **after removing any `ESC[201~`** and C0/C1 controls (except `\t`, `\n`, `\r`) from the payload. When the app has not enabled it, multi-line paste confirmation applies (setting).
3. **Answerback and reports** (DA/DSR/XTVERSION/XTGETTCAP): responses are fixed, short, contain no user data; unsolicited reports are never generated. Dangerous queries like `DECRQSS` are limited to a safe subset.
4. **OSC 52**: default policy `write-only with notification`; **read is denied** unless the user sets *ask* and approves per request; payload ≤ 1 MB.
5. **Dropped/pasted paths** are quoted per shell and have control characters percent-escaped before insertion.
6. **Hyperlinks (OSC 8) and detected URLs**: open only on Ctrl/Cmd+click; allowed schemes `http`, `https`, `mailto`, `file` (file only when host matches local); the target URL is shown in the status bar on hover; `ssh://` and custom schemes are off by default.
7. **File-path click-to-open** substitutes into an editor command template using argv arrays — never through a shell string.
8. **Inline images**: dimension and byte caps; decode on a worker with timeouts; reject decompression bombs (pixel limit, e.g., 64 MP); cache cap per `docs/03`.
9. **Escape parsing robustness**: parameter count and value caps (e.g., 32 params, values saturate at `u16::MAX`), OSC payload cap (default 1 MiB), DCS/APC/PM/SOS strings capped and discarded when unknown; no unbounded growth from an unterminated sequence (watchdog resets the parser after the cap).
10. **Terminfo and environment**: only documented variables are injected; the daemon never forwards its own sensitive env to children beyond what the user's shell would get.
11. **Resource limits per session**: max scrollback memory (RAM + disk spill cap, default 256 MB disk), max image cache, max hyperlink table, max title length — prevents a single hostile program from exhausting the machine.

## 4. Privacy

- **No telemetry. No analytics. No crash upload.** Crash reports are written locally; the UI offers "Copy diagnostics" and links to the issue template.
- Network use in v1.0: update check only (user-disableable), a plain HTTPS GET to the release manifest (no identifiers beyond `User-Agent: Ferrum/<version>` and the OS/arch).
- Logs never contain terminal content, keystrokes, command lines, environment values or file contents (`docs/05` §8).
- Spill files and image clips live in the per-user runtime dir with 0600 permissions and are wiped on daemon exit (best-effort overwrite not promised; documented).
- Exported `.ferrum` archives contain no scrollback unless explicitly selected; the dialog says so.

## 5. Updates and distribution integrity

- Update manifest and artifacts are signed with an ed25519 **minisign** key; the public key is embedded in the binary; the private key is held offline/in a protected CI secret with a documented rotation plan.
- The updater verifies signature and SHA-256, downloads over HTTPS (`rustls`), stages the update, and applies on next restart. Downgrades are refused unless the user opts in.
- **Signing status (decided 2026-11-20):** no Apple Developer ID and no Windows code-signing certificate yet. macOS builds are ad-hoc signed only; Windows builds are unsigned. Integrity therefore rests on: SHA-256 sums and detached minisign signatures published with each release, build-provenance attestations, hash-pinned package-manager manifests (Homebrew cask, Scoop, winget), and the updater's own signature verification. OS-level trust (Gatekeeper/SmartScreen) is not available until accounts exist; the docs say so plainly. Linux packages are checksummed and carry repository/AUR signatures where supported.
- GitHub release artifacts carry build-provenance attestations; SBOM (CycloneDX) is published.

## 6. Supply chain

- `Cargo.lock` committed; builds in CI use `--locked`.
- `cargo deny` (licenses allow-list, banned crates, duplicate-version warnings, unknown registries/git sources denied) on every PR; `cargo audit` nightly; Dependabot/Renovate weekly.
- No `build.rs` network access; build scripts of dependencies are reviewed when added (`cargo-vet` optional).
- Third-party code (fixtures, fonts, themes) tracked in `THIRD_PARTY.md` with licenses; only permissive licenses (MIT, Apache-2.0, BSD, ISC, OFL for fonts, Unicode, Zlib, MPL-2.0 for unmodified deps).
- GitHub Actions pinned by SHA; workflow permissions default `contents: read`, elevated per job; secrets only in the release workflow; no `pull_request_target` with untrusted code.

## 7. Phase 2 secret handling (design constraints now)

- Keys live only in the encrypted vault (`docs/13` §6); never in config TOML, logs, command lines, or environment variables of unrelated sessions.
- Injecting a key into a child process is explicit per agent and uses the child's environment only, never argv.
- Agents are untrusted code: they run as the user's own processes inside PTYs; Ferrum does not claim sandboxing. The permission model of each agent CLI remains the agent's own.
- The built-in MCP server listens only on the owner-only local socket, requires the per-session token (`FERRUM_MCP_TOKEN`), and each tool call is attributed to a session in the audit log (local, size-capped).

## 8. Vulnerability handling

`SECURITY.md`: private reporting via GitHub Security Advisories; acknowledgment within 72 h; fix targets: critical 7 days, high 30 days; coordinated disclosure; advisory published with the patched release.
