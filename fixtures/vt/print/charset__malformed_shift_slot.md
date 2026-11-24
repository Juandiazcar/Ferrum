# charset / malformed_shift_slot

A shift to an undesignated set prints ASCII.

Reference: xterm ctlseqs: SS2. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 4e | ESC N | SS2 (G2 is ASCII) |
| 71 71 | q q | plain |
