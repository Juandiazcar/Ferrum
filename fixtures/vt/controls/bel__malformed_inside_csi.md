# bel / malformed_inside_csi

BEL inside an unfinished CSI is executed and the sequence continues.

Reference: xterm ctlseqs: BEL. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 | ESC [ 3 | CSI 3 (unfinished) |
| 07 | <07> | BEL executes |
| 31 6d | 1 m | completes the sequence: SGR 31 |
| 78 | x | text |
