# vpa / basic

VPA moves to an absolute row.

Reference: xterm ctlseqs: VPA. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 0d 0a 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e 2e | . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . <0d> <0a> . . . . . . . . . . . . . . . . . . . . | background dots |
| 1b 5b 33 3b 35 48 | ESC [ 3 ; 5 H | CUP 3;5 |
| 1b 5b 32 64 | ESC [ 2 d | VPA 2 |
| 58 | X | marker |
