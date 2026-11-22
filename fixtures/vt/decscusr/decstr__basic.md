# decstr / basic

DECSTR resets modes, attributes, margins and charsets but keeps the screen.

Reference: xterm ctlseqs: DECSTR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 33 31 6d | ESC [ 1 ; 3 1 m | bold red |
| 1b 5b 34 68 | ESC [ 4 h | IRM |
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region |
| 1b 28 30 | ESC ( 0 | line drawing |
| 6b 65 65 70 | k e e p | text |
| 1b 5b 21 70 | ESC [ ! p | DECSTR |
| 78 | x | text |
