# cpr / origin_mode

In origin mode the row is relative to the scroll region.

Reference: xterm ctlseqs: CPR, DECOM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 35 72 | ESC [ 3 ; 5 r | region 3-5 |
| 1b 5b 3f 36 68 | ESC [ ? 6 h | origin mode |
| 1b 5b 32 3b 32 48 | ESC [ 2 ; 2 H | CUP 2;2 |
| 1b 5b 36 6e | ESC [ 6 n | CPR |
