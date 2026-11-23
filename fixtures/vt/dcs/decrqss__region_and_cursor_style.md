# decrqss / region_and_cursor_style

DECRQSS r and SP q report the scroll region and the cursor style.

Reference: xterm ctlseqs: DECRQSS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region 2-4 |
| 1b 5b 35 20 71 | ESC [ 5 SP q | blinking bar |
| 1b 50 24 71 72 1b 5c | ESC P $ q r ESC \ | DECRQSS r |
| 1b 50 24 71 20 71 1b 5c | ESC P $ q SP q ESC \ | DECRQSS SP q |
