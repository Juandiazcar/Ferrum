# 13 — Phase 2: Agents, Providers, Usage, Vault, MCP

**Status: designed now, built after v1.0.** The terminal must not depend on any of this. When disabled, Phase 2 costs zero RAM and zero CPU (crates not instantiated, no threads, no sockets).

## 1. Principles

1. **A terminal first.** Official agent CLIs (Claude Code, Codex CLI, Gemini CLI, OpenCode, etc.) run inside normal PTY sessions, exactly as in any terminal. Ferrum adds metadata and plumbing around them; it does not reimplement them.
2. **No credential laundering.** Ferrum does not extract, reuse, or proxy subscription OAuth tokens from other vendors' CLIs. Subscriptions are used by running the vendor's own CLI. API keys are the user's own, entered by them, stored in the vault.
3. **Honest meters.** Every number shown has a precision label: `exact` (from an official API), `estimated` (derived from local logs/tokens), or `unknown`. Never present an estimate as exact.
4. **Additive seams.** Phase 2 attaches through the seams reserved in `docs/02` §9.

## 2. Concepts

| Concept | Definition |
|---|---|
| **Agent session** | A session of kind `Agent`. Variant A: *CLI agent* — a PTY running a CLI command with an attached agent profile. Variant B: *API agent* — an in-app conversation backed by a provider/model (rendered as a chat/terminal-like view) |
| **Provider** | A source of models: `Anthropic`, `OpenAI-compatible` (OpenRouter, MiniMax, Qwen/DashScope, Z.ai/GLM, DeepSeek, Moonshot, local Ollama/LM Studio…), plus official CLIs as "subscription providers" |
| **Account** | Credentials + endpoint for a provider (API key in the vault; subscription accounts are represented by "installed CLI + login state detected", no secrets held) |
| **Agent template** | Name, kind, command or provider/model, system prompt, env, working directory root, permissions hints, color/icon |
| **Workflow** | A user-created wiring of agents (e.g., planner → implementer → reviewer) |
| **Bus** | In-daemon typed message bus; the MCP server and the UI are clients |

Mixing agents and models in one project is first-class: a project's layout may contain terminals, CLI agents and API agents side by side.

## 3. Agent ↔ agent communication (fast, automatic)

**Mechanism: a built-in MCP server.**

- `ferrumd` hosts an MCP server on a local owner-only socket (`ferrum mcp-bridge` is a tiny stdio shim so any MCP client can connect).
- When Ferrum launches a CLI agent, it **auto-registers** the server for that run (via the agent's supported mechanism: per-invocation config flag/file or env var; documented per agent) and sets `FERRUM_MCP_TOKEN`, `FERRUM_SESSION_ID`.
- Tools exposed (initial): `ferrum.list_sessions`, `ferrum.send_message(to, text)`, `ferrum.read_messages()`, `ferrum.read_screen(session, lines)` (permissioned), `ferrum.send_input(session, text)` (permissioned, off by default), `ferrum.create_session(template)`, `ferrum.get_project()`.
- Permissions: per-agent allow-list set in the UI; destructive tools (`send_input`, `create_session`) require explicit user approval the first time per agent per project, remembered per setting.
- The user can also drive it manually: "send this pane's last output to agent B" as a UI action.
- API agents use the same tools in-process.

Why MCP: it is the common denominator already supported by the major agent CLIs; no per-agent adapter code beyond registration.

## 4. Providers and usage meters

### 4.1 `UsageSource` trait

```rust
pub trait UsageSource {
    fn id(&self) -> ProviderId;
    fn poll_interval(&self) -> Duration;        // e.g., 60 s; 0 = event-driven only
    fn read(&mut self) -> Result<UsageReading, UsageError>;
}

pub struct UsageReading {
    pub precision: Precision,                   // Exact | Estimated | Unknown
    pub windows: Vec<UsageWindow>,              // e.g., "5h", "weekly", "monthly credits"
    pub as_of: SystemTime,
}
pub struct UsageWindow { label: String, used_fraction: Option<f32>, used: Option<Quantity>, limit: Option<Quantity>, resets_at: Option<SystemTime> }
```

### 4.2 Adapter plan (to be **verified against current vendor behavior at P2-04 start**)

| Source | Likely approach | Expected precision |
|---|---|---|
| OpenRouter | Official API: key info and credits endpoints | exact |
| Anthropic API (own key) | Usage/cost from response headers (rate-limit headers) and local accounting; org usage API if the user's key type allows | exact for rate limits, estimated for spend |
| Claude subscription via Claude Code | Parse the CLI's local session logs/state for token usage and window timing; read any official usage command's output when the CLI offers one | estimated (exact only if an official source exists) |
| ChatGPT subscription via Codex CLI | Parse local session records, which may include rate-limit snapshots emitted by the service | estimated or exact depending on what the records contain |
| MiniMax, Qwen, Z.ai/GLM, DeepSeek, Moonshot | Per-vendor: official balance/usage endpoints where they exist; otherwise local token accounting from API responses | exact if endpoint exists, else estimated |
| Local models | N/A (tokens/s only) | — |

**Open items (must be researched at the start of P2-04, not assumed):** which of these vendors expose an official usage/quota endpoint for *subscription* plans; whether reading CLI-local logs is permitted by each vendor's terms; stability of those file formats. Where no legitimate source exists the UI shows "usage unavailable" rather than inventing a number.

### 4.3 Alerts and routing
- Thresholds per window (e.g., 80 %, 95 %) → notification + sidebar badge.
- Optional rule: "when provider X reaches N %, suggest/switch the agent to provider Y" — suggestion by default, automatic only if enabled per agent.

### 4.4 Status bar
Compact meters: provider icon + bar + remaining time (`4h12m`), colored by threshold, tooltip with precision label and last update. Implemented as `StatusItem` plug-ins registered at runtime.

## 5. Natural-language project generation

Input: "A helpdesk project with a React frontend and a Node backend, plus a reviewer agent."
Output (as a preview the user can edit before confirming): project name, directories (created or selected), session recipe (terminals + agents), agent templates, suggested git init and scaffolding commands. Executed by an agent session (chosen by the user) with an explicit "plan → approve → run" step; Ferrum never runs generated commands without approval.

## 6. Vault

- Contents: API keys/tokens and per-agent secrets. Not used for anything else.
- Format: single file `vault.bin`: header (version, KDF params, salt, nonce) + ChaCha20-Poly1305 ciphertext of a serialized map; atomic writes.
- Key management: a random 256-bit master key stored in the OS keychain (`keyring`); the vault file alone is useless without it. **Export** re-encrypts under an Argon2id-derived key from a user passphrase so the vault can move to another PC (`.ferrum` archive section).
- In memory: secrets in `zeroize`-on-drop containers; never logged; never placed in argv; injected into a child's environment only on explicit per-agent opt-in.
- UI: keys entered in a masked field inside Ferrum ("inside the terminal app" as requested), shown as `••••last4`, copy disabled by default.

## 7. Networking (decided at P2-01)

- Prefer blocking I/O on a small, bounded thread pool (`ureq` + `rustls`) if streaming responses and a handful of concurrent agents suffice; only introduce `tokio` (currently banned by `deny.toml`) if profiling or protocol needs (SSE streaming at scale, many concurrent connections) justify it, isolating it in `ferrum-providers` on its own named thread(s) so the terminal path remains runtime-free.

## 8. Memory and CPU rules for Phase 2

- Disabled → nothing instantiated (verify with `memcheck` baseline unchanged).
- Enabled → incremental budget table appended to `docs/03` (target: ≤ +6 MB for vault+usage+MCP with 3 agents; API chat views bounded by their own scrollback rules).
- No polling faster than 30 s for usage unless a session is actively running that provider.

## 9. Testing

- Fake provider servers (local HTTP) for adapters; recorded fixtures for log parsers with tolerant parsing and version tags.
- MCP conformance tests with a reference client; permission-prompt UI logic tests.
- Vault: known-answer tests, tamper detection, rotation, export/import across OS.
- Security tests: secrets never appear in logs/argv/env of unrelated sessions (integration test scanning process environments and log files).
