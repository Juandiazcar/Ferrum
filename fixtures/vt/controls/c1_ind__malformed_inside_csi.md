# c1_ind / malformed_inside_csi

The 8-bit IND (0x84) inside an unfinished CSI is dropped and the CSI completes as CUF 3; nothing of the sequence prints.

Reference: xterm ctlseqs: IND (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 1b 5b 33 | ESC [ 3 | CSI 3 (unfinished) |
| 84 | <84> | 8-bit IND inside the CSI |
| 43 | C | final byte of the interrupted CSI |
| 62 | b | text |
