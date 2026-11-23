# mouse_exclusive / basic

Only the last enabled mouse tracking level stays set.

Reference: xterm ctlseqs: mouse tracking. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 30 30 68 | ESC [ ? 1 0 0 0 h | normal |
| 1b 5b 3f 31 30 30 33 68 | ESC [ ? 1 0 0 3 h | any-event |
| 1b 5b 3f 31 30 30 36 68 | ESC [ ? 1 0 0 6 h | SGR encoding |
| 61 | a | text |
