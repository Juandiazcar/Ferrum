# decrqss / sgr

DECRQSS m reports the current SGR.

Reference: xterm ctlseqs: DECRQSS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 33 3b 34 3a 33 3b 33 38 3b 32 3b 31 3b 32 3b 33 3b 34 38 3b 35 3b 39 6d | ESC [ 1 ; 3 ; 4 : 3 ; 3 8 ; 2 ; 1 ; 2 ; 3 ; 4 8 ; 5 ; 9 m | bold italic curly underline, rgb fg, palette bg |
| 1b 50 24 71 6d 1b 5c | ESC P $ q m ESC \ | DECRQSS m |
