# tbc / malformed_mode

An unknown TBC mode is ignored.

Reference: xterm ctlseqs: TBC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 35 67 | ESC [ 5 g | TBC 5 |
| 09 | <09> | HT still uses default stops |
| 58 | X | marker |
