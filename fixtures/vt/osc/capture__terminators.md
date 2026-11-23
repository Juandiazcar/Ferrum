# capture / terminators

BEL and ESC \ both end an OSC; text after them prints normally.

Reference: xterm ctlseqs: OSC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 32 3b 6f 6e 65 07 | ESC ] 2 ; o n e <07> | BEL terminated |
| 61 | a | text |
| 1b 5d 32 3b 74 77 6f 1b 5c | ESC ] 2 ; t w o ESC \ | ST terminated |
| 62 | b | text |
