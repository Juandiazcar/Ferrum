# ed / bce

ED fills with the current background color (BCE) but not other attributes.

Reference: xterm ctlseqs: ED, BCE. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 0d 0a 62 62 62 62 62 62 62 62 62 62 0d 0a 63 63 63 63 63 63 63 63 63 63 0d 0a 64 64 64 64 64 64 64 64 64 64 | a a a a a a a a a a <0d> <0a> b b b b b b b b b b <0d> <0a> c c c c c c c c c c <0d> <0a> d d d d d d d d d d | four rows |
| 1b 5b 32 3b 34 48 | ESC [ 2 ; 4 H | CUP 2;4 |
| 1b 5b 31 3b 34 34 6d | ESC [ 1 ; 4 4 m | bold + blue background |
| 1b 5b 32 4a | ESC [ 2 J | ED 2 |
