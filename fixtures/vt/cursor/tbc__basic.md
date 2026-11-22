# tbc / basic

TBC 0 clears the stop at the cursor; TBC 3 clears all.

Reference: xterm ctlseqs: TBC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 39 47 | ESC [ 9 G | CHA 9: a default stop |
| 1b 5b 67 | ESC [ g | TBC 0 |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 09 | <09> | HT skips the cleared stop |
| 58 | X | marker |
| 1b 5b 33 67 | ESC [ 3 g | TBC 3: clear all |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 09 | <09> | HT goes to the last column |
| 59 | Y | marker |
