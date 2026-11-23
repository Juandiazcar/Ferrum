# hts / basic

ESC H sets a tab stop at the cursor.

Reference: xterm ctlseqs: HTS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 47 | ESC [ 4 G | CHA 4 |
| 1b 48 | ESC H | HTS |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 09 | <09> | HT |
| 58 | X | marker |
