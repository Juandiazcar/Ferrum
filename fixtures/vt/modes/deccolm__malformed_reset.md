# deccolm / malformed_reset

DECRST 3 does the same and clears the 132-column flag.

Reference: xterm ctlseqs: DECCOLM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 33 68 | ESC [ ? 3 h | DECSET 3 |
| 68 65 6c 6c 6f | h e l l o | text |
| 1b 5b 3f 33 6c | ESC [ ? 3 l | DECRST 3 |
| 78 | x | text |
