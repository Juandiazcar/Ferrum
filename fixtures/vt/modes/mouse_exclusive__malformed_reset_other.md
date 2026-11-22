# mouse_exclusive / malformed_reset_other

Resetting a level that is not active changes nothing.

Reference: xterm ctlseqs: mouse tracking. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 33 68 | ESC [ ? 1 0 0 3 h | any-event |
| 1b 5b 3f 31 30 30 30 6c | ESC [ ? 1 0 0 0 l | reset normal (not active) |
| 61 | a | text |
