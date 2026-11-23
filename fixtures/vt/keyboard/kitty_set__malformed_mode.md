# kitty_set / malformed_mode

An unknown set mode is ignored.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3d 31 3b 39 75 | ESC [ = 1 ; 9 u | unknown mode 9 |
| 1b 5b 3f 75 | ESC [ ? u | query: 0 |
