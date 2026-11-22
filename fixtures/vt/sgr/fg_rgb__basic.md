# fg_rgb / basic

SGR 38;2;r;g;b selects a 24-bit foreground.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 38 3b 32 3b 31 30 3b 32 30 3b 33 30 6d | ESC [ 3 8 ; 2 ; 1 0 ; 2 0 ; 3 0 m | SGR 38;2;10;20;30 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
