# osc12 / malformed_spec

A malformed OSC 12 spec is ignored.

Reference: xterm ctlseqs: OSC 12. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 32 3b 72 67 62 3a 7a 7a 2f 30 30 2f 30 30 07 | ESC ] 1 2 ; r g b : z z / 0 0 / 0 0 <07> | invalid |
| 1b 5d 31 32 3b 3f 07 | ESC ] 1 2 ; ? <07> | query |
