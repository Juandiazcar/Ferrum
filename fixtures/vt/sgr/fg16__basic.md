# fg16 / basic

SGR 30-37 select a normal foreground; 39 restores the default.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 31 6d | ESC [ 3 1 m | SGR 31 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
