# mouse_normal / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 1000. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 30 3b 39 39 39 39 39 3b 31 30 30 30 6c | ESC [ ? 1 0 0 0 ; 9 9 9 9 9 ; 1 0 0 0 l | DECRST 1000, unknown, 1000 |
| 1b 5b 3f 31 30 30 30 3b 31 30 30 30 68 | ESC [ ? 1 0 0 0 ; 1 0 0 0 h | DECSET twice |
| 1b 5b 3f 31 30 30 30 68 | ESC [ ? 1 0 0 0 h | DECSET |
| 1b 5b 3f 31 30 30 30 6c | ESC [ ? 1 0 0 0 l | DECRST |
| 61 62 | a b | text |
