# ff / malformed_inside_esc

FF inside an unfinished ESC sequence is executed.

Reference: xterm ctlseqs: FF. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b | ESC | ESC |
| 0c | <0c> | FF executes |
| 37 | 7 | completes DECSC |
| 78 | x | text |
