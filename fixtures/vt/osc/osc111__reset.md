# osc111 / reset

OSC 111 resets the background color set by OSC 11 to the theme.

Reference: xterm ctlseqs: OSC 111. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 31 3b 23 32 32 32 32 32 32 07 | ESC ] 1 1 ; # 2 2 2 2 2 2 <07> | set bg |
| 1b 5d 31 31 31 07 | ESC ] 1 1 1 <07> | reset bg |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query bg |
