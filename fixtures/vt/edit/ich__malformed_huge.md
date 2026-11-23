# ich / malformed_huge

A huge ICH blanks the rest of the row; the cut text is lost.

Reference: xterm ctlseqs: ICH. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 | a b c d e f | text |
| 1b 5b 33 47 | ESC [ 3 G | CHA 3 |
| 1b 5b 39 39 39 39 40 | ESC [ 9 9 9 9 @ | ICH 9999 |
