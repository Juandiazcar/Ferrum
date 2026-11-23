# osc104 / reset_one

OSC 104 with indices resets those entries to the theme.

Reference: xterm ctlseqs: OSC 104. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 34 3b 31 3b 23 30 31 30 32 30 33 3b 32 3b 23 30 34 30 35 30 36 07 | ESC ] 4 ; 1 ; # 0 1 0 2 0 3 ; 2 ; # 0 4 0 5 0 6 <07> | set 1 and 2 |
| 1b 5d 31 30 34 3b 31 07 | ESC ] 1 0 4 ; 1 <07> | reset 1 |
| 1b 5d 34 3b 31 3b 3f 3b 32 3b 3f 07 | ESC ] 4 ; 1 ; ? ; 2 ; ? <07> | query both |
