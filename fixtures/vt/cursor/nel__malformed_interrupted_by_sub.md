# nel / malformed_interrupted_by_sub

SUB cancels ESC before E: no NEL; the E that follows is plain text.

Reference: xterm ctlseqs: NEL, SUB. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 1b | ESC | ESC |
| 1a | <1a> | SUB cancels the sequence |
| 45 | E | text, not NEL |
| 62 | b | text |
