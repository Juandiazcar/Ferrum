# el / mode1

EL 1 erases start of line to cursor.

Reference: xterm ctlseqs: EL. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 0d 0a 62 62 62 62 62 62 62 62 62 62 0d 0a 63 63 63 63 63 63 63 63 63 63 0d 0a 64 64 64 64 64 64 64 64 64 64 | a a a a a a a a a a <0d> <0a> b b b b b b b b b b <0d> <0a> c c c c c c c c c c <0d> <0a> d d d d d d d d d d | four rows |
| 1b 5b 32 3b 34 48 | ESC [ 2 ; 4 H | CUP 2;4 |
| 1b 5b 31 4b | ESC [ 1 K | EL 1 |
