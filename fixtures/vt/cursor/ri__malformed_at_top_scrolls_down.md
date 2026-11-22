# ri / malformed_at_top_scrolls_down

RI on the top row scrolls the screen down.

Reference: xterm ctlseqs: RI. Terminal size: 20x3.

| hex | bytes | meaning |
|---|---|---|
| 6f 6e 65 0d 0a 74 77 6f 0d 0a 74 68 72 65 65 | o n e <0d> <0a> t w o <0d> <0a> t h r e e | three rows |
| 1b 5b 31 3b 31 48 | ESC [ 1 ; 1 H | CUP 1;1 |
| 1b 4d | ESC M | RI scrolls down |
| 6e 65 77 | n e w | text |
