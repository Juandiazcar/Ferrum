# ed / mode3

ED 3 erases the saved lines (history) and leaves the screen.

Reference: xterm ctlseqs: ED. Terminal size: 20x3.

| hex | bytes | meaning |
|---|---|---|
| 31 0d 0a 32 0d 0a 33 0d 0a 34 0d 0a 35 | 1 <0d> <0a> 2 <0d> <0a> 3 <0d> <0a> 4 <0d> <0a> 5 | five rows on a three-row screen: two go to history |
| 1b 5b 33 4a | ESC [ 3 J | ED 3 |
