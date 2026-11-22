# ich / basic

ICH inserts blanks at the cursor, pushing text right.

Reference: xterm ctlseqs: ICH. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 | a b c d e f | text |
| 1b 5b 33 47 | ESC [ 3 G | CHA 3 |
| 1b 5b 32 40 | ESC [ 2 @ | ICH 2 |
