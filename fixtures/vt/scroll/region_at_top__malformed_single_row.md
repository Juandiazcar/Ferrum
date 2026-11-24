# region_at_top / malformed_single_row

A one-row region is rejected.

Reference: xterm: scroll region. Terminal size: 20x4.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 32 72 | ESC [ 2 ; 2 r | region of one row: ignored |
| 61 0d 0a 62 | a <0d> <0a> b | text |
