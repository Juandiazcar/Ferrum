# mouse_x10 / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 9. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 39 3b 39 39 39 39 39 3b 39 6c | ESC [ ? 9 ; 9 9 9 9 9 ; 9 l | DECRST 9, unknown, 9 |
| 1b 5b 3f 39 3b 39 68 | ESC [ ? 9 ; 9 h | DECSET twice |
| 1b 5b 3f 39 68 | ESC [ ? 9 h | DECSET |
| 1b 5b 3f 39 6c | ESC [ ? 9 l | DECRST |
| 61 62 | a b | text |
