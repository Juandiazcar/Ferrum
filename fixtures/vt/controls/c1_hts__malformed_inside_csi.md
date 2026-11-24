# c1_hts / malformed_inside_csi

The 8-bit HTS (0x88) inside an unfinished CSI is dropped (no stop is set) and the CSI completes as CHA 1; nothing of the sequence prints.

Reference: xterm ctlseqs: HTS (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 67 | ESC [ 3 g | clear all stops |
| 1b 5b 35 47 | ESC [ 5 G | CHA 5 |
| 1b 5b 31 | ESC [ 1 | CSI 1 (unfinished) |
| 88 | <88> | 8-bit HTS inside the CSI |
| 47 | G | final byte of the interrupted CSI |
| 0d 09 | <0d> <09> | CR HT: no stop, so to the last column |
| 58 | X | marker |
