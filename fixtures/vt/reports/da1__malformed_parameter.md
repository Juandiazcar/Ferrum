# da1 / malformed_parameter

A non-zero parameter is still answered with the same fixed text.

Reference: xterm ctlseqs: Primary DA. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 37 63 | ESC [ 7 c | DA1 with parameter 7 |
| 1b 5b 31 3b 32 3b 33 63 | ESC [ 1 ; 2 ; 3 c | DA1 with extra parameters |
