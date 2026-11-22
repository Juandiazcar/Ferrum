# dch / basic

DCH deletes cells, pulling text left.

Reference: xterm ctlseqs: DCH. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 | a b c d e f | text |
| 1b 5b 32 47 | ESC [ 2 G | CHA 2 |
| 1b 5b 32 50 | ESC [ 2 P | DCH 2 |
