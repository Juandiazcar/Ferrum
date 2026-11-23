# ind / malformed_interrupted_by_can

CAN cancels ESC before D: no IND; the D that follows is plain text.

Reference: xterm ctlseqs: IND, CAN. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 1b | ESC | ESC |
| 18 | <18> | CAN cancels the sequence |
| 44 | D | text, not IND |
| 62 | b | text |
