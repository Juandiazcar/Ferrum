# 1b 5b 33 3b 36 73|CSI 3 ; 6 s|SCOSC with parameters (not DECSLRM) / 61 62 63 64 65 66 67 68 69 6a 6b|a b c d e f g h i j k|eleven characters wrap at column 10, not 6

Mode 69 and DECSLRM are ignored: text still wraps at the last column.

Reference: xterm ctlseqs: DECLRMM, DECSLRM. Terminal size: 10x3.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 36 39 68 | CSI ? 69 h | left and right margin mode, ignored |
| 1b 5b 33 3b 36 73 | CSI 3 ; 6 s | SCOSC with parameters (not DECSLRM) |
| 61 62 63 64 65 66 67 68 69 6a 6b | a b c d e f g h i j k | eleven characters wrap at column 10, not 6 |
