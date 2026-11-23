# 88|<88>|HTS at column 3 / 0d|CR|home

The 8-bit HTS control (0x88) sets a tab stop at the cursor column.

Reference: xterm ctlseqs: HTS (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 | a b c | text |
| 88 | <88> | HTS at column 3 |
| 0d | CR | home |
| 78 09 09 | x HT HT | the first tab reaches the new stop at column 3, the second the default stop at 8 |
| 79 | y | text at column 8 |
