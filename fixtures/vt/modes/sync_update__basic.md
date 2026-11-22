# sync_update / basic

Synchronized output (state only; the daemon holds damage back).

Reference: synchronized output spec. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 30 32 36 68 | ESC [ ? 2 0 2 6 h | DECSET 2026 |
| 61 62 | a b | text |
