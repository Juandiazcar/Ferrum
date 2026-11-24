# underline_styles / basic

SGR 4:n selects the underline style (1 single, 2 double, 3 curly, 4 dotted, 5 dashed, 0 none).

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 3a 33 6d | ESC [ 4 : 3 m | SGR 4:3 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
