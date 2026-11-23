# decckm / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: DECCKM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 3b 39 39 39 39 39 3b 31 6c | ESC [ ? 1 ; 9 9 9 9 9 ; 1 l | DECRST 1, unknown, 1 |
| 1b 5b 3f 31 3b 31 68 | ESC [ ? 1 ; 1 h | DECSET twice |
| 1b 5b 3f 31 68 | ESC [ ? 1 h | DECSET |
| 1b 5b 3f 31 6c | ESC [ ? 1 l | DECRST |
| 61 62 | a b | text |
