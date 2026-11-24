# cub / malformed_zero_or_huge

A huge count stops at column 0.

Reference: xterm ctlseqs: CUB. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e | . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . | background dots |
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 39 39 39 39 44 | ESC [ 9 9 9 9 D | CUB 9999 |
| 58 | X | marker |
