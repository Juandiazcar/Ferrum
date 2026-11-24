# lnm / malformed_reset

RM 20 restores plain LF.

Reference: xterm ctlseqs: LNM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 30 68 | ESC [ 2 0 h | SM 20 |
| 1b 5b 32 30 6c | ESC [ 2 0 l | RM 20 |
| 61 62 0a 63 64 | a b <0a> c d | staircase |
