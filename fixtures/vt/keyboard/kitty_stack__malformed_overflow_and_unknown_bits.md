# kitty_stack / malformed_overflow_and_unknown_bits

Unknown flag bits are dropped; a full stack drops its oldest entry.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3e 32 35 35 75 | ESC [ > 2 5 5 u | push with unknown bits |
| 1b 5b 3f 75 | ESC [ ? u | query: only 31 survives |
| 1b 5b 3c 39 39 75 | ESC [ < 9 9 u | pop everything |
| 1b 5b 3f 75 | ESC [ ? u | query: 0 |
