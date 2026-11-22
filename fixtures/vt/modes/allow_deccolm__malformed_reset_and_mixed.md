# allow_deccolm / malformed_reset_and_mixed

Reset, repeated set, and an unknown mode in the same sequence.

Reference: xterm ctlseqs: mode 40. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 34 30 3b 39 39 39 39 39 3b 34 30 6c | ESC [ ? 4 0 ; 9 9 9 9 9 ; 4 0 l | DECRST 40, unknown, 40 |
| 1b 5b 3f 34 30 3b 34 30 68 | ESC [ ? 4 0 ; 4 0 h | DECSET twice |
| 1b 5b 3f 34 30 68 | ESC [ ? 4 0 h | DECSET |
| 1b 5b 3f 34 30 6c | ESC [ ? 4 0 l | DECRST |
| 61 62 | a b | text |
