# osc104 / malformed_index

A non-numeric or out-of-range index in OSC 104 is skipped.

Reference: xterm ctlseqs: OSC 104. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 34 3b 31 3b 23 30 31 30 32 30 33 07 | ESC ] 4 ; 1 ; # 0 1 0 2 0 3 <07> | set 1 |
| 1b 5d 31 30 34 3b 78 3b 39 39 39 3b 31 07 | ESC ] 1 0 4 ; x ; 9 9 9 ; 1 <07> | bad, bad, reset 1 |
| 1b 5d 34 3b 31 3b 3f 07 | ESC ] 4 ; 1 ; ? <07> | query 1 |
