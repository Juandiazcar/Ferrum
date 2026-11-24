# osc112 / reset

OSC 112 resets the cursor color set by OSC 12 to the theme.

Reference: xterm ctlseqs: OSC 112. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 32 3b 23 33 33 33 33 33 33 07 | ESC ] 1 2 ; # 3 3 3 3 3 3 <07> | set cursor |
| 1b 5d 31 31 32 07 | ESC ] 1 1 2 <07> | reset cursor |
| 1b 5d 31 32 3b 3f 07 | ESC ] 1 2 ; ? <07> | query cursor |
