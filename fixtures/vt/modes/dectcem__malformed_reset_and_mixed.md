# dectcem / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: DECTCEM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 35 3b 39 39 39 39 39 3b 32 35 6c | ESC [ ? 2 5 ; 9 9 9 9 9 ; 2 5 l | DECRST 25, unknown, 25 |
| 1b 5b 3f 32 35 3b 32 35 68 | ESC [ ? 2 5 ; 2 5 h | DECSET twice |
| 1b 5b 3f 32 35 68 | ESC [ ? 2 5 h | DECSET |
| 1b 5b 3f 32 35 6c | ESC [ ? 2 5 l | DECRST |
| 61 62 | a b | text |
