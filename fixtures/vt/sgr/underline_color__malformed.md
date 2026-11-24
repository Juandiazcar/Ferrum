# underline_color / malformed

An invalid index is ignored.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 35 38 3b 35 3b 39 39 39 6d | ESC [ 5 8 ; 5 ; 9 9 9 m | SGR 58;5;999 |
| 61 62 | a b | text |
