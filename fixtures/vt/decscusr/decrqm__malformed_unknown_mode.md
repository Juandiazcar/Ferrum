# decrqm / malformed_unknown_mode

A mode the terminal does not know is reported as 0.

Reference: xterm ctlseqs: DECRQM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 39 39 39 39 24 70 | ESC [ ? 9 9 9 9 $ p | query 9999 |
| 1b 5b 24 70 | ESC [ $ p | query without a number |
| 1b 5b 39 39 39 24 70 | ESC [ 9 9 9 $ p | ANSI query 999 |
