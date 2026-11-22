# colon_rgb / basic

SGR 38:2::r:g:b and 38:2:r:g:b select a 24-bit color with colon separators.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 38 3a 32 3a 3a 31 30 3a 32 30 3a 33 30 3b 34 38 3a 32 3a 34 3a 35 3a 36 6d | ESC [ 3 8 : 2 : : 1 0 : 2 0 : 3 0 ; 4 8 : 2 : 4 : 5 : 6 m | SGR 38:2::10:20:30;48:2:4:5:6 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
