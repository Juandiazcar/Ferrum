# fg256 / malformed

An index above 255 is ignored.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 38 3b 35 3b 33 30 30 6d | ESC [ 3 8 ; 5 ; 3 0 0 m | SGR 38;5;300 |
| 61 62 | a b | text |
