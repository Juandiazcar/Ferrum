# decsc_decrc / basic

ESC 7 saves the cursor and attributes; ESC 8 restores them.

Reference: xterm ctlseqs: DECSC, DECRC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 35 48 | ESC [ 2 ; 5 H | CUP 2;5 |
| 1b 37 | ESC 7 | DECSC |
| 1b 5b 35 3b 31 30 48 | ESC [ 5 ; 1 0 H | CUP 5;10 |
| 61 | a | text |
| 1b 38 | ESC 8 | DECRC |
| 58 | X | marker at the saved position |
