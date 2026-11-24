# italic / basic

SGR 3 sets italic; SGR 23 clears it.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 6d | ESC [ 3 m | SGR 3 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
