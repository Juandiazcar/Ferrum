# kitty_screens / per_screen_stacks

The main and alternate screens keep separate stacks.

Reference: kitty keyboard protocol. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3e 31 75 | ESC [ > 1 u | push 1 on the main screen |
| 1b 5b 3f 31 30 34 39 68 | ESC [ ? 1 0 4 9 h | alternate screen |
| 1b 5b 3f 75 | ESC [ ? u | query: 0 |
| 1b 5b 3e 38 75 | ESC [ > 8 u | push 8 |
| 1b 5b 3f 75 | ESC [ ? u | query: 8 |
| 1b 5b 3f 31 30 34 39 6c | ESC [ ? 1 0 4 9 l | main screen |
| 1b 5b 3f 75 | ESC [ ? u | query: 1 |
