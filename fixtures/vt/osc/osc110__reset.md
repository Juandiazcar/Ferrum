# osc110 / reset

OSC 110, 111 and 112 reset the foreground, background and cursor colors to the theme.

Reference: xterm ctlseqs: OSC 110. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 30 3b 23 31 31 31 31 31 31 07 | ESC ] 1 0 ; # 1 1 1 1 1 1 <07> | fg |
| 1b 5d 31 31 3b 23 32 32 32 32 32 32 07 | ESC ] 1 1 ; # 2 2 2 2 2 2 <07> | bg |
| 1b 5d 31 32 3b 23 33 33 33 33 33 33 07 | ESC ] 1 2 ; # 3 3 3 3 3 3 <07> | cursor |
| 1b 5d 31 31 30 07 | ESC ] 1 1 0 <07> | reset fg |
| 1b 5d 31 31 31 07 | ESC ] 1 1 1 <07> | reset bg |
| 1b 5d 31 31 32 07 | ESC ] 1 1 2 <07> | reset cursor |
| 1b 5d 31 30 3b 3f 07 | ESC ] 1 0 ; ? <07> | query |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query |
| 1b 5d 31 32 3b 3f 07 | ESC ] 1 2 ; ? <07> | query |
