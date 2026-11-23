# history / malformed_region_not_at_top

Scrolling a region that does not start at the top never touches history.

Reference: docs/02 section 3.2. Terminal size: 20x4.

| hex | bytes | meaning |
|---|---|---|
| 31 0d 0a 32 0d 0a 33 0d 0a 34 | 1 <0d> <0a> 2 <0d> <0a> 3 <0d> <0a> 4 | four rows |
| 1b 5b 32 3b 33 72 | ESC [ 2 ; 3 r | region 2-3 |
| 1b 5b 33 3b 31 48 | ESC [ 3 ; 1 H | CUP 3;1 |
| 0a 0a 0a | <0a> <0a> <0a> | three line feeds scroll the region |
