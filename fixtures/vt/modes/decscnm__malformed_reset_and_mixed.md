# decscnm / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: DECSCNM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 35 3b 39 39 39 39 39 3b 35 6c | ESC [ ? 5 ; 9 9 9 9 9 ; 5 l | DECRST 5, unknown, 5 |
| 1b 5b 3f 35 3b 35 68 | ESC [ ? 5 ; 5 h | DECSET twice |
| 1b 5b 3f 35 68 | ESC [ ? 5 h | DECSET |
| 1b 5b 3f 35 6c | ESC [ ? 5 l | DECRST |
| 61 62 | a b | text |
