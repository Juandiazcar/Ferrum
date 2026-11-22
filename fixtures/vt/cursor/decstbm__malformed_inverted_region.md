# decstbm / malformed_inverted_region

A region with top >= bottom is ignored; no parameters resets to the full screen.

Reference: xterm ctlseqs: DECSTBM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 3b 32 72 | ESC [ 4 ; 2 r | inverted: ignored |
| 61 | a | text |
| 1b 5b 32 3b 33 72 | ESC [ 2 ; 3 r | valid region |
| 1b 5b 72 | ESC [ r | reset to full |
| 62 | b | text |
