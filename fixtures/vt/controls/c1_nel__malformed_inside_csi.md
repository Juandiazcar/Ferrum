# c1_nel / malformed_inside_csi

The 8-bit NEL (0x85) inside an unfinished CSI is dropped and the CSI completes as CUF 3; nothing of the sequence prints.

Reference: xterm ctlseqs: NEL (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 1b 5b 33 | ESC [ 3 | CSI 3 (unfinished) |
| 85 | <85> | 8-bit NEL inside the CSI |
| 43 | C | final byte of the interrupted CSI |
| 62 | b | text |
