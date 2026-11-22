# ris / basic

ESC c resets the terminal to its power-on state (and discards the history).

Reference: xterm ctlseqs: RIS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 0d 0a 62 62 62 62 62 62 62 62 62 62 0d 0a 63 63 63 63 63 63 63 63 63 63 0d 0a 64 64 64 64 64 64 64 64 64 64 | a a a a a a a a a a <0d> <0a> b b b b b b b b b b <0d> <0a> c c c c c c c c c c <0d> <0a> d d d d d d d d d d | four rows |
| 1b 5b 32 3b 34 48 | ESC [ 2 ; 4 H | CUP 2;4 |
| 1b 5b 31 3b 33 31 6d | ESC [ 1 ; 3 1 m | bold red |
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region |
| 1b 63 | ESC c | RIS |
| 78 | x | text |
