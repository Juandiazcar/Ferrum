# save_cursor_1048 / malformed_restore_first

Restoring without a save homes the cursor.

Reference: xterm ctlseqs: mode 1048. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 33 48 | ESC [ 3 ; 3 H | CUP 3;3 |
| 1b 5b 3f 31 30 34 38 6c | ESC [ ? 1 0 4 8 l | restore without save |
| 58 | X | marker |
