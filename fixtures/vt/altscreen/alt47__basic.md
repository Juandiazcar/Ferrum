# alt47 / basic

Mode 47 switches to a blank alternate screen and back; the primary screen is untouched.

Reference: xterm ctlseqs: alternate screen. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 70 72 69 6d 61 72 79 | p r i m a r y | text on the primary screen |
| 1b 5b 3f 34 37 68 | ESC [ ? 4 7 h | enter alternate screen |
| 61 6c 74 | a l t | text on the alternate screen |
| 1b 5b 3f 34 37 6c | ESC [ ? 4 7 l | leave |
| 21 | ! | back on the primary screen |
