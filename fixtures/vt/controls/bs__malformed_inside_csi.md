# bs / malformed_inside_csi

BS inside an unfinished CSI is executed and the sequence continues.

Reference: xterm ctlseqs: BS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 | a b c | text |
| 1b 5b 32 | ESC [ 2 | CSI 2 (unfinished) |
| 08 | <08> | BS executes |
| 44 | D | completes the sequence: CUB 2 |
| 58 | X | marker |
