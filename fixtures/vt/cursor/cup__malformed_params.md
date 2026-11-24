# cup / malformed_params

Missing, zero and out-of-range parameters are defaults or clamped; extra parameters are ignored.

Reference: xterm ctlseqs: CUP. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 48 | ESC [ H | CUP home |
| 61 | a | text |
| 1b 5b 30 3b 30 48 | ESC [ 0 ; 0 H | CUP 0;0 = home |
| 62 | b | text |
| 1b 5b 39 39 3b 39 39 48 | ESC [ 9 9 ; 9 9 H | CUP clamped |
| 63 | c | text |
| 1b 5b 32 3b 33 3b 34 3b 35 48 | ESC [ 2 ; 3 ; 4 ; 5 H | extra parameters ignored |
| 64 | d | text |
