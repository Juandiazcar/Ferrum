# sgr_mouse / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 1006. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 36 3b 39 39 39 39 39 3b 31 30 30 36 6c | ESC [ ? 1 0 0 6 ; 9 9 9 9 9 ; 1 0 0 6 l | DECRST 1006, unknown, 1006 |
| 1b 5b 3f 31 30 30 36 3b 31 30 30 36 68 | ESC [ ? 1 0 0 6 ; 1 0 0 6 h | DECSET twice |
| 1b 5b 3f 31 30 30 36 68 | ESC [ ? 1 0 0 6 h | DECSET |
| 1b 5b 3f 31 30 30 36 6c | ESC [ ? 1 0 0 6 l | DECRST |
| 61 62 | a b | text |
