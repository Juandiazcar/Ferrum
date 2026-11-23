# blink / basic

SGR 5 and 6 set blink; SGR 25 clears it.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 35 6d | ESC [ 5 m | SGR 5 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
