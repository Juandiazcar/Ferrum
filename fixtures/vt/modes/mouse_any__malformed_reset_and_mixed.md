# mouse_any / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 1003. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 33 3b 39 39 39 39 39 3b 31 30 30 33 6c | ESC [ ? 1 0 0 3 ; 9 9 9 9 9 ; 1 0 0 3 l | DECRST 1003, unknown, 1003 |
| 1b 5b 3f 31 30 30 33 3b 31 30 30 33 68 | ESC [ ? 1 0 0 3 ; 1 0 0 3 h | DECSET twice |
| 1b 5b 3f 31 30 30 33 68 | ESC [ ? 1 0 0 3 h | DECSET |
| 1b 5b 3f 31 30 30 33 6c | ESC [ ? 1 0 0 3 l | DECRST |
| 61 62 | a b | text |
