# osc111 / malformed_aborted_by_can

CAN aborts OSC 111 before its terminator: the background is not reset.

Reference: xterm ctlseqs: OSC 111, CAN. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 31 3b 23 32 32 32 32 32 32 07 | ESC ] 1 1 ; # 2 2 2 2 2 2 <07> | set bg |
| 1b 5d 31 31 31 | ESC ] 1 1 1 | OSC 111 (unterminated) |
| 18 | <18> | CAN aborts it |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query bg |
