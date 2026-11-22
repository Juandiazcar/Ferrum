# cup / basic

CUP moves to row;column (1-based).

Reference: xterm ctlseqs: CUP. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e | . . . . <0d> <0a> . . . . <0d> <0a> . . . . | dots |
| 1b 5b 32 3b 33 48 | ESC [ 2 ; 3 H | CUP 2;3 |
| 58 | X | marker |
