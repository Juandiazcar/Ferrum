# cr / malformed_inside_csi

CR inside an unfinished CSI is executed.

Reference: xterm ctlseqs: CR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 | a b c | text |
| 1b 5b 31 | ESC [ 1 | CSI 1 (unfinished) |
| 0d | <0d> | CR executes |
| 43 | C | CUF 1 |
| 58 | X | text |
