# decstr / malformed_wrong_intermediate

CSI p without the ! is not DECSTR.

Reference: xterm ctlseqs: DECSTR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 6d | ESC [ 1 m | bold |
| 1b 5b 70 | ESC [ p | CSI p: not DECSTR |
| 78 | x | text |
