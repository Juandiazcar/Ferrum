# lf / malformed_inside_csi

LF inside an unfinished CSI is executed and the sequence continues.

Reference: xterm ctlseqs: LF. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 | ESC [ 2 | CSI 2 (unfinished) |
| 0a | <0a> | LF executes |
| 43 | C | completes the sequence: CUF 2 |
| 78 | x | text |
