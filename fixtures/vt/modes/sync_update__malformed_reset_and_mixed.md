# sync_update / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: synchronized output spec. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 30 32 36 3b 39 39 39 39 39 3b 32 30 32 36 6c | ESC [ ? 2 0 2 6 ; 9 9 9 9 9 ; 2 0 2 6 l | DECRST 2026, unknown, 2026 |
| 1b 5b 3f 32 30 32 36 3b 32 30 32 36 68 | ESC [ ? 2 0 2 6 ; 2 0 2 6 h | DECSET twice |
| 1b 5b 3f 32 30 32 36 68 | ESC [ ? 2 0 2 6 h | DECSET |
| 1b 5b 3f 32 30 32 36 6c | ESC [ ? 2 0 2 6 l | DECRST |
| 61 62 | a b | text |
