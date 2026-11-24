# nel / edge_at_bottom_scrolls

NEL on the bottom row scrolls.

Reference: xterm ctlseqs: NEL. Terminal size: 20x3.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 3b 31 48 | ESC [ 3 ; 1 H | CUP 3;1 |
| 6c 61 73 74 | l a s t | text |
| 1b 45 | ESC E | NEL scrolls |
| 6e 65 77 | n e w | text |
