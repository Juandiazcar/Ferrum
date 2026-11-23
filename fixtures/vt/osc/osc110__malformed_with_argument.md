# osc110 / malformed_with_argument

Resets ignore arguments and a reset of an unchanged color reports nothing.

Reference: xterm ctlseqs: OSC 110. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 31 30 3b 69 67 6e 6f 72 65 64 07 | ESC ] 1 1 0 ; i g n o r e d <07> | reset fg with junk |
| 1b 5d 31 31 31 07 | ESC ] 1 1 1 <07> | reset bg (already default) |
