# alt1049 / cursor_saved_and_restored

1049 saves the cursor on entry and restores it on exit.

Reference: xterm ctlseqs: mode 1049. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 31 3b 33 31 6d | ESC [ 1 ; 3 1 m | bold red |
| 1b 5b 3f 31 30 34 39 68 | ESC [ ? 1 0 4 9 h | enter |
| 1b 5b 31 3b 31 48 | ESC [ 1 ; 1 H | home |
| 61 6c 74 | a l t | text |
| 1b 5b 3f 31 30 34 39 6c | ESC [ ? 1 0 4 9 l | leave |
| 58 | X | marker at the saved position, red bold |
