# region_at_top / basic

A region that starts at row 1 sends the rows it scrolls out to history.

Reference: xterm: scroll region. Terminal size: 20x4.

| hex | bytes | meaning |
|---|---|---|
| 31 0d 0a 32 0d 0a 33 0d 0a 34 | 1 <0d> <0a> 2 <0d> <0a> 3 <0d> <0a> 4 | four rows |
| 1b 5b 31 3b 33 72 | ESC [ 1 ; 3 r | region 1-3 |
| 1b 5b 33 3b 31 48 | ESC [ 3 ; 1 H | CUP 3;1 |
| 0a | <0a> | LF scrolls the region |
| 5a | Z | marker |
