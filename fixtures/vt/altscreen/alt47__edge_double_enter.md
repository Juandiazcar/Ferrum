# alt47 / edge_double_enter

Entering twice and leaving twice is harmless.

Reference: xterm ctlseqs: alternate screen. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 34 37 68 | ESC [ ? 4 7 h | enter |
| 6f 6e 65 | o n e | text |
| 1b 5b 3f 34 37 68 | ESC [ ? 4 7 h | enter again (no-op) |
| 74 77 6f | t w o | text |
| 1b 5b 3f 34 37 6c | ESC [ ? 4 7 l | leave |
| 1b 5b 3f 34 37 6c | ESC [ ? 4 7 l | leave again (no-op) |
