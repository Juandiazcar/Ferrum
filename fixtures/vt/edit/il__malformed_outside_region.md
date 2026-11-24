# il / malformed_outside_region

IL with the cursor outside the scroll region does nothing.

Reference: xterm ctlseqs: IL. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 0d 0a 62 62 62 62 62 62 62 62 62 62 0d 0a 63 63 63 63 63 63 63 63 63 63 0d 0a 64 64 64 64 64 64 64 64 64 64 | a a a a a a a a a a <0d> <0a> b b b b b b b b b b <0d> <0a> c c c c c c c c c c <0d> <0a> d d d d d d d d d d | four rows |
| 1b 5b 32 3b 34 48 | ESC [ 2 ; 4 H | CUP 2;4 |
| 1b 5b 31 3b 32 72 | ESC [ 1 ; 2 r | region rows 1-2 |
| 1b 5b 34 3b 31 48 | ESC [ 4 ; 1 H | CUP 4;1 (outside) |
| 1b 5b 4c | ESC [ L | IL 1 |
