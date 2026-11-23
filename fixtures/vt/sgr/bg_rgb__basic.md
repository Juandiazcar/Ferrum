# bg_rgb / basic

SGR 48;2;r;g;b selects a 24-bit background.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 38 3b 32 3b 31 3b 32 3b 33 6d | ESC [ 4 8 ; 2 ; 1 ; 2 ; 3 m | SGR 48;2;1;2;3 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
