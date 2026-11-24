# scosc_scorc / basic

CSI s saves and CSI u restores the cursor (no left/right margin mode).

Reference: xterm ctlseqs: SCOSC, SCORC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 35 48 | ESC [ 2 ; 5 H | CUP 2;5 |
| 1b 5b 73 | ESC [ s | save |
| 1b 5b 35 3b 31 30 48 | ESC [ 5 ; 1 0 H | CUP 5;10 |
| 1b 5b 75 | ESC [ u | restore |
| 58 | X | marker |
