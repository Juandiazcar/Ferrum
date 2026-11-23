# charset / line_drawing

ESC ( 0 selects DEC special graphics for G0; ESC ( B returns to ASCII.

Reference: xterm ctlseqs: SCS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 28 30 | ESC ( 0 | G0 = DEC special graphics |
| 6c 71 6b | l q k | box top |
| 1b 28 42 | ESC ( B | G0 = ASCII |
| 6c 71 6b | l q k | plain |
