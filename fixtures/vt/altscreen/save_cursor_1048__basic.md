# save_cursor_1048 / basic

Mode 1048 saves and restores the cursor like DECSC / DECRC.

Reference: xterm ctlseqs: mode 1048. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 33 48 | ESC [ 2 ; 3 H | CUP 2;3 |
| 1b 5b 3f 31 30 34 38 68 | ESC [ ? 1 0 4 8 h | save |
| 1b 5b 35 3b 35 48 | ESC [ 5 ; 5 H | CUP 5;5 |
| 1b 5b 3f 31 30 34 38 6c | ESC [ ? 1 0 4 8 l | restore |
| 58 | X | marker |
