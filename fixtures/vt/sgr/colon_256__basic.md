# colon_256 / basic

SGR 38:5:n selects a palette color with colon separators.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 38 3a 35 3a 39 39 6d | ESC [ 3 8 : 5 : 9 9 m | SGR 38:5:99 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
