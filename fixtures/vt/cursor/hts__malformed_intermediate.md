# hts / malformed_intermediate

ESC with an intermediate before H is not HTS.

Reference: xterm ctlseqs: HTS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 47 | ESC [ 4 G | CHA 4 |
| 1b 23 48 | ESC # H | ESC # H: not HTS |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 09 | <09> | HT to the default stop |
| 58 | X | marker |
