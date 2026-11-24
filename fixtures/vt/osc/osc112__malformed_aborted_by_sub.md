# osc112 / malformed_aborted_by_sub

SUB aborts OSC 112 before its terminator: the cursor color is not reset.

Reference: xterm ctlseqs: OSC 112, SUB. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 32 3b 23 33 33 33 33 33 33 07 | ESC ] 1 2 ; # 3 3 3 3 3 3 <07> | set cursor |
| 1b 5d 31 31 32 | ESC ] 1 1 2 | OSC 112 (unterminated) |
| 1a | <1a> | SUB aborts it |
| 1b 5d 31 32 3b 3f 07 | ESC ] 1 2 ; ? <07> | query cursor |
