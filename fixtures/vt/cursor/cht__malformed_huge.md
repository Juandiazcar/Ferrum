# cht / malformed_huge

A huge CHT stops at the last column.

Reference: xterm ctlseqs: CHT. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 39 39 39 39 49 | ESC [ 9 9 9 9 I | CHT 9999 |
| 58 | X | marker |
