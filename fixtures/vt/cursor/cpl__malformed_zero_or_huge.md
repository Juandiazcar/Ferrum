# cpl / malformed_zero_or_huge

A zero count means 1.

Reference: xterm ctlseqs: CPL. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e | . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . | background dots |
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 30 46 | ESC [ 0 F | CPL 0 |
| 58 | X | marker |
