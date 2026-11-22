# decsca / basic

DECSCA 1 protects the cells printed afterwards from selective erase (DECSED/DECSEL); ED/EL still erase them.

Reference: xterm ctlseqs: DECSCA. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 22 71 | ESC [ 1 " q | DECSCA 1 |
| 70 72 6f 74 65 63 74 65 64 | p r o t e c t e d | text |
| 1b 5b 30 22 71 | ESC [ 0 " q | DECSCA 0 |
| 70 6c 61 69 6e | p l a i n | text |
| 1b 5b 3f 32 4b | ESC [ ? 2 K | DECSEL 2: keeps the protected cells |
