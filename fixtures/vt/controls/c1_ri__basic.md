# 8d|<8d>|RI back to row 0 / 62|b|text at column 1 of row 0

The 8-bit RI control (0x8D) moves up one row.

Reference: xterm ctlseqs: RI (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 0d 0a 63 | a CR LF c | row 1 holds c |
| 8d | <8d> | RI back to row 0 |
| 62 | b | text at column 1 of row 0 |
