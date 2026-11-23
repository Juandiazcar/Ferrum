# kitty_set / modes

CSI = flags ; mode u replaces (1), adds (2) or removes (3) bits of the active flags.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3d 31 75 | ESC [ = 1 u | replace with 1 |
| 1b 5b 3f 75 | ESC [ ? u | query: 1 |
| 1b 5b 3d 36 3b 32 75 | ESC [ = 6 ; 2 u | add 6 |
| 1b 5b 3f 75 | ESC [ ? u | query: 7 |
| 1b 5b 3d 32 3b 33 75 | ESC [ = 2 ; 3 u | remove 2 |
| 1b 5b 3f 75 | ESC [ ? u | query: 5 |
| 1b 5b 3d 31 36 75 | ESC [ = 1 6 u | replace with 16 |
| 1b 5b 3f 75 | ESC [ ? u | query: 16 |
