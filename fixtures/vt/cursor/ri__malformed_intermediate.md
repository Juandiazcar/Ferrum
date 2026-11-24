# ri / malformed_intermediate

ESC with an intermediate before M is not RI and is ignored.

Reference: xterm ctlseqs: RI. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 31 48 | ESC [ 2 ; 1 H | CUP 2;1 |
| 1b 23 4d | ESC # M | ESC # M: not RI |
| 58 | X | marker stays on row 2 |
