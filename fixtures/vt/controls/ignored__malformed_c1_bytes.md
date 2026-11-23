# ignored / malformed_c1_bytes

Raw C1 bytes that are not handled are executed as controls and print nothing.

Reference: xterm ctlseqs: C1. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 80 81 9b 9d | <80> <81> <9b> <9d> | raw C1 bytes |
| 62 | b | text |
