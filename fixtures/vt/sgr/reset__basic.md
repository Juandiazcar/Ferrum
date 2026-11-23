# reset / basic

SGR 0 (and an empty SGR) resets every attribute and color.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 33 31 3b 34 34 6d | ESC [ 1 ; 3 1 ; 4 4 m | SGR 1;31;44 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
