# unknown / basic

Unknown SGR codes are ignored and do not stop later ones.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 37 37 37 3b 33 6d | ESC [ 1 ; 7 7 7 ; 3 m | SGR 1;777;3 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
