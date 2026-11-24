# underline / basic

SGR 4 sets a single underline; SGR 24 clears it.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 6d | ESC [ 4 m | SGR 4 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
