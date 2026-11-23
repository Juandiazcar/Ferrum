# app_keypad / malformed_intermediate

ESC # > is not DECKPNM.

Reference: xterm ctlseqs: DECKPNM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 3d | ESC = | DECKPAM |
| 1b 23 3e | ESC # > | ESC # > |
| 61 | a | text |
