# bracketed_paste / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: bracketed paste. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 30 30 34 3b 39 39 39 39 39 3b 32 30 30 34 6c | ESC [ ? 2 0 0 4 ; 9 9 9 9 9 ; 2 0 0 4 l | DECRST 2004, unknown, 2004 |
| 1b 5b 3f 32 30 30 34 3b 32 30 30 34 68 | ESC [ ? 2 0 0 4 ; 2 0 0 4 h | DECSET twice |
| 1b 5b 3f 32 30 30 34 68 | ESC [ ? 2 0 0 4 h | DECSET |
| 1b 5b 3f 32 30 30 34 6c | ESC [ ? 2 0 0 4 l | DECRST |
| 61 62 | a b | text |
