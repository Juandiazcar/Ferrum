# strike / basic

SGR 9 sets strikethrough; SGR 29 clears it.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 39 6d | ESC [ 9 m | SGR 9 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
