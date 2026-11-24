# att610 / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: cursor blink. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 32 3b 39 39 39 39 39 3b 31 32 6c | ESC [ ? 1 2 ; 9 9 9 9 9 ; 1 2 l | DECRST 12, unknown, 12 |
| 1b 5b 3f 31 32 3b 31 32 68 | ESC [ ? 1 2 ; 1 2 h | DECSET twice |
| 1b 5b 3f 31 32 68 | ESC [ ? 1 2 h | DECSET |
| 1b 5b 3f 31 32 6c | ESC [ ? 1 2 l | DECRST |
| 61 62 | a b | text |
