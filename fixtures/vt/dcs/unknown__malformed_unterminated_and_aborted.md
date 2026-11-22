# unknown / malformed_unterminated_and_aborted

A DCS cut off by CAN or by another escape sequence ends there.

Reference: xterm ctlseqs: DCS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 50 6a 75 6e 6b 6a 75 6e 6b | ESC P j u n k j u n k | unterminated |
| 18 | <18> | CAN |
| 61 | a | text |
| 1b 50 6d 6f 72 65 | ESC P m o r e | another |
| 1b 5b 32 3b 33 48 | ESC [ 2 ; 3 H | CUP ends it |
| 62 | b | text |
