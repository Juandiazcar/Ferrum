# cpr / basic

DSR 6 reports the cursor position (row;column, 1-based).

Reference: xterm ctlseqs: CPR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 36 6e | ESC [ 6 n | CPR |
| 1b 5b 31 3b 31 48 | ESC [ 1 ; 1 H | home |
| 1b 5b 36 6e | ESC [ 6 n | CPR |
