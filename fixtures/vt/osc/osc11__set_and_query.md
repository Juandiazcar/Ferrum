# osc11 / set_and_query

OSC 11 sets or queries the default background.

Reference: xterm ctlseqs: OSC 11. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 31 3b 72 67 62 3a 31 32 2f 33 34 2f 35 36 07 | ESC ] 1 1 ; r g b : 1 2 / 3 4 / 5 6 <07> | set background |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query background |
