# decsc_decrc / malformed_intermediate

ESC with an intermediate before 8 is not DECRC and is ignored.

Reference: xterm ctlseqs: DECSC, DECRC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 33 48 | ESC [ 3 ; 3 H | CUP 3;3 |
| 1b 37 | ESC 7 | DECSC |
| 1b 5b 31 3b 31 30 48 | ESC [ 1 ; 1 0 H | CUP 1;10 |
| 1b 20 38 | ESC SP 8 | ESC SP 8: not DECRC |
| 58 | X | marker stays at 1;10 |
