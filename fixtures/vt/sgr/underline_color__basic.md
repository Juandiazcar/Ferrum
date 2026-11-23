# underline_color / basic

SGR 58 sets the underline color (palette or RGB); SGR 59 restores the default.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 3b 35 38 3b 32 3b 32 35 35 3b 30 3b 30 6d | ESC [ 4 ; 5 8 ; 2 ; 2 5 5 ; 0 ; 0 m | SGR 4;58;2;255;0;0 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
