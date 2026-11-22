# capture / malformed_aborted_by_can

CAN aborts an OSC without executing it; later text prints.

Reference: xterm ctlseqs: OSC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 32 3b 6e 65 76 65 72 | ESC ] 2 ; n e v e r | OSC start |
| 18 | <18> | CAN |
| 76 69 73 69 62 6c 65 | v i s i b l e | text |
