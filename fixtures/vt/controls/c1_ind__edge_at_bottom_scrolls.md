# c1_ind / edge_at_bottom_scrolls

IND on the last row scrolls the screen into the history.

Reference: xterm ctlseqs: IND (C1). Terminal size: 6x3.

| hex | bytes | meaning |
|---|---|---|
| 61 0d 0a 62 0d 0a 63 | a CR LF b CR LF c | three rows |
| 84 | <84> | IND at the bottom scrolls |
| 64 | d | text |
