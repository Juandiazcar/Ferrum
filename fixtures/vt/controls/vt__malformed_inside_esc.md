# vt / malformed_inside_esc

VT inside an unfinished ESC sequence is executed.

Reference: xterm ctlseqs: VT. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b | ESC | ESC |
| 0b | <0b> | VT executes |
| 37 | 7 | completes DECSC |
| 78 | x | text |
