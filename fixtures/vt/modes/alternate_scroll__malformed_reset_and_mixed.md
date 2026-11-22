# alternate_scroll / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 1007. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 37 3b 39 39 39 39 39 3b 31 30 30 37 6c | ESC [ ? 1 0 0 7 ; 9 9 9 9 9 ; 1 0 0 7 l | DECRST 1007, unknown, 1007 |
| 1b 5b 3f 31 30 30 37 3b 31 30 30 37 68 | ESC [ ? 1 0 0 7 ; 1 0 0 7 h | DECSET twice |
| 1b 5b 3f 31 30 30 37 68 | ESC [ ? 1 0 0 7 h | DECSET |
| 1b 5b 3f 31 30 30 37 6c | ESC [ ? 1 0 0 7 l | DECRST |
| 61 62 | a b | text |
