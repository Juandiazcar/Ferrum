# osc10 / malformed_spec

An invalid color spec leaves the color alone.

Reference: xterm ctlseqs: OSC 10. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 30 3b 62 6c 75 69 73 68 07 | ESC ] 1 0 ; b l u i s h <07> | invalid |
| 1b 5d 31 30 3b 3f 07 | ESC ] 1 0 ; ? <07> | query |
