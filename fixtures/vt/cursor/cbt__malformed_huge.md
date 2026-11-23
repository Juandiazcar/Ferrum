# cbt / malformed_huge

A huge CBT stops at column 0.

Reference: xterm ctlseqs: CBT. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 35 47 | ESC [ 5 G | CHA 5 |
| 1b 5b 39 39 39 39 5a | ESC [ 9 9 9 9 Z | CBT 9999 |
| 58 | X | marker |
