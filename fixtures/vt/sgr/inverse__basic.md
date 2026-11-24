# inverse / basic

SGR 7 sets inverse video; SGR 27 clears it.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 37 6d | ESC [ 7 m | SGR 7 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
