# decstbm / basic

DECSTBM sets the scroll region; scrolling stays inside it and the cursor homes.

Reference: xterm ctlseqs: DECSTBM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 31 0d 0a 32 0d 0a 33 0d 0a 34 0d 0a 35 0d 0a 36 | 1 <0d> <0a> 2 <0d> <0a> 3 <0d> <0a> 4 <0d> <0a> 5 <0d> <0a> 6 | six rows |
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | DECSTBM 2;4 |
| 1b 5b 34 3b 31 48 | ESC [ 4 ; 1 H | CUP 4;1 |
| 1b 44 | ESC D | IND at the region bottom scrolls the region |
| 58 | X | marker |
