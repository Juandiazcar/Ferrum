# cpr / malformed_wrong_parameter

DSR 6 with extra parameters is answered like DSR 6; DSR 0 and 9 are ignored.

Reference: xterm ctlseqs: CPR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 36 3b 31 6e | ESC [ 6 ; 1 n | DSR 6;1 |
| 1b 5b 30 6e | ESC [ 0 n | DSR 0: ignored |
| 1b 5b 39 6e | ESC [ 9 n | DSR 9: ignored |
