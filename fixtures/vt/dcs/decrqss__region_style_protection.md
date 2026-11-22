# decrqss / region_style_protection

DECRQSS r, SP q and " q report the scroll region, cursor style and protection.

Reference: xterm ctlseqs: DECRQSS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region 2-4 |
| 1b 5b 35 20 71 | ESC [ 5 SP q | blinking bar |
| 1b 5b 31 22 71 | ESC [ 1 " q | protected |
| 1b 50 24 71 72 1b 5c | ESC P $ q r ESC \ | DECRQSS r |
| 1b 50 24 71 20 71 1b 5c | ESC P $ q SP q ESC \ | DECRQSS SP q |
| 1b 50 24 71 22 71 1b 5c | ESC P $ q " q ESC \ | DECRQSS " q |
