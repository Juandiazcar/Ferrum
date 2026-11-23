# decom / malformed_reset

DECRST 6 homes the cursor to the top of the screen.

Reference: xterm ctlseqs: DECOM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region 2-4 |
| 1b 5b 3f 36 68 | ESC [ ? 6 h | DECSET 6 |
| 1b 5b 3f 36 6c | ESC [ ? 6 l | DECRST 6 |
| 41 | A | marker |
