# bg16 / basic

SGR 40-47 select a normal background; 49 restores the default.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 32 6d | ESC [ 4 2 m | SGR 42 |
| 61 62 | a b | text |
| 1b 5b 30 6d | ESC [ 0 m | SGR 0 |
| 63 | c | text |
