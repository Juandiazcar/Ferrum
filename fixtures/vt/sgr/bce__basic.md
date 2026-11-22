# bce / basic

Erase uses the current background color (BCE) and nothing else.

Reference: xterm ctlseqs: BCE. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 34 31 6d | ESC [ 1 ; 4 1 m | bold, red background |
| 1b 5b 32 4b | ESC [ 2 K | EL 2 |
| 78 | x | text |
