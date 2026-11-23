# scosc_scorc / malformed_restore_without_save

CSI u without a save homes the cursor.

Reference: xterm ctlseqs: SCORC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 33 48 | ESC [ 3 ; 3 H | CUP 3;3 |
| 1b 5b 75 | ESC [ u | restore |
| 58 | X | marker |
