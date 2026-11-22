# osc4 / query

OSC 4 with ? replies with the current color of the entry.

Reference: xterm ctlseqs: OSC 4. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 34 3b 31 3b 3f 07 | ESC ] 4 ; 1 ; ? <07> | query entry 1 |
| 1b 5d 34 3b 32 35 35 3b 3f 07 | ESC ] 4 ; 2 5 5 ; ? <07> | query entry 255 |
| 1b 5d 34 3b 31 3b 23 31 30 32 30 33 30 3b 31 3b 3f 07 | ESC ] 4 ; 1 ; # 1 0 2 0 3 0 ; 1 ; ? <07> | set then query |
