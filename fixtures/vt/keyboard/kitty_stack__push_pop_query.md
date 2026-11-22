# kitty_stack / push_pop_query

CSI > flags u pushes, CSI < u pops, CSI ? u reports the active flags.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 75 | ESC [ ? u | query: 0 |
| 1b 5b 3e 31 75 | ESC [ > 1 u | push 1 |
| 1b 5b 3e 35 75 | ESC [ > 5 u | push 5 |
| 1b 5b 3f 75 | ESC [ ? u | query: 5 |
| 1b 5b 3c 75 | ESC [ < u | pop 1 |
| 1b 5b 3f 75 | ESC [ ? u | query: 1 |
| 1b 5b 3c 35 75 | ESC [ < 5 u | pop more than there are |
| 1b 5b 3f 75 | ESC [ ? u | query: 0 |
