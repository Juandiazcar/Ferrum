# bg256 / basic

SGR 48;5;n selects a palette background.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 38 3b 35 3b 31 37 6d | ESC [ 4 8 ; 5 ; 1 7 m | SGR 48;5;17 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
