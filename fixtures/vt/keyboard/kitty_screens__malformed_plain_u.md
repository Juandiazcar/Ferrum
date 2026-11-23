# kitty_screens / malformed_plain_u

CSI u without an intermediate restores the cursor and does not touch the stack.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3e 33 75 | ESC [ > 3 u | push 3 |
| 1b 5b 32 3b 32 48 | ESC [ 2 ; 2 H | CUP 2;2 |
| 1b 5b 75 | ESC [ u | restore cursor (nothing saved: home) |
| 1b 5b 3f 75 | ESC [ ? u | query: 3 |
