# decsc_decrc / edge_restore_without_save

DECRC without a save homes the cursor.

Reference: xterm ctlseqs: DECRC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 33 48 | ESC [ 3 ; 3 H | CUP 3;3 |
| 1b 38 | ESC 8 | DECRC |
| 58 | X | marker at home |
