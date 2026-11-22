# params / malformed_too_many

More than 32 parameters: the extra ones are dropped, the rest applies.

Reference: xterm ctlseqs: SGR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 3b 31 6d | ESC [ 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 ; 1 m | 40 times SGR 1 |
| 61 | a | text |
