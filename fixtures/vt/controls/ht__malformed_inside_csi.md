# ht / malformed_inside_csi

HT inside an unfinished CSI is executed and the sequence continues.

Reference: xterm ctlseqs: HT. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 | ESC [ 2 | CSI 2 (unfinished) |
| 09 | <09> | HT executes |
| 43 | C | completes the sequence: CUF 2 |
| 58 | X | marker |
