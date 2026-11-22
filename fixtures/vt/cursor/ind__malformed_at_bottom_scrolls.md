# ind / malformed_at_bottom_scrolls

IND on the bottom row scrolls the screen and fills history.

Reference: xterm ctlseqs: IND. Terminal size: 20x3.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 31 48 | ESC [ 3 ; 1 H | CUP 3;1: last row |
| 6c 61 73 74 | l a s t | text |
| 1b 44 | ESC D | IND scrolls |
| 6e 65 77 | n e w | text |
