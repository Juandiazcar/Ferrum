# osc11 / malformed_spec

An empty OSC 11 changes nothing.

Reference: xterm ctlseqs: OSC 11. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 31 07 | ESC ] 1 1 <07> | no argument |
| 1b 5d 31 31 3b 07 | ESC ] 1 1 ; <07> | empty spec |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query |
