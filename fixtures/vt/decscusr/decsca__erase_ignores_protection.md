# decsca / erase_ignores_protection

ED and EL erase protected cells too.

Reference: xterm ctlseqs: DECSCA. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 22 71 | ESC [ 1 " q | DECSCA 1 |
| 70 72 6f 74 65 63 74 65 64 | p r o t e c t e d | text |
| 1b 5b 32 4b | ESC [ 2 K | EL 2 |
