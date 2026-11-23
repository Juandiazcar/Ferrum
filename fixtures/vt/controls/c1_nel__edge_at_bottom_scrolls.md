# c1_nel / edge_at_bottom_scrolls

NEL on the last row scrolls the screen.

Reference: xterm ctlseqs: NEL (C1). Terminal size: 6x3.

| hex | bytes | meaning |
|---|---|---|
| 61 0d 0a 62 0d 0a 63 | a CR LF b CR LF c | three rows |
| 85 | <85> | NEL at the bottom scrolls |
| 64 | d | text |
