# fg256 / basic

SGR 38;5;n selects a palette foreground.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 38 3b 35 3b 32 30 30 6d | ESC [ 3 8 ; 5 ; 2 0 0 m | SGR 38;5;200 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
