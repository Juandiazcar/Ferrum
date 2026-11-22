# bg256 / malformed

A truncated sequence is ignored.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 38 3b 35 6d | ESC [ 4 8 ; 5 m | SGR 48;5 |
| 61 62 | a b | text |
