# cuu / malformed_zero_or_huge

A zero or missing count means 1; too many parameters are bounded.

Reference: xterm ctlseqs: CUU. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e | . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . | background dots |
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 30 41 | ESC [ 0 A | CUU 0 |
| 58 | X | marker |
