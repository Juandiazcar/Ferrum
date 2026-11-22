# bg_rgb / malformed

A channel above 255: ignored.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 38 3b 32 3b 31 3b 32 3b 39 39 39 6d | ESC [ 4 8 ; 2 ; 1 ; 2 ; 9 9 9 m | SGR 48;2;1;2;999 |
| 61 62 | a b | text |
