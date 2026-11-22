# mouse_button / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 1002. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 32 3b 39 39 39 39 39 3b 31 30 30 32 6c | ESC [ ? 1 0 0 2 ; 9 9 9 9 9 ; 1 0 0 2 l | DECRST 1002, unknown, 1002 |
| 1b 5b 3f 31 30 30 32 3b 31 30 30 32 68 | ESC [ ? 1 0 0 2 ; 1 0 0 2 h | DECSET twice |
| 1b 5b 3f 31 30 30 32 68 | ESC [ ? 1 0 0 2 h | DECSET |
| 1b 5b 3f 31 30 30 32 6c | ESC [ ? 1 0 0 2 l | DECRST |
| 61 62 | a b | text |
