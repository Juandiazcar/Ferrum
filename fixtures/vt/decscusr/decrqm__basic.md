# decrqm / basic

DECRQM reports a mode as set (1), reset (2) or unknown (0).

Reference: xterm ctlseqs: DECRQM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 30 30 34 68 | ESC [ ? 2 0 0 4 h | bracketed paste on |
| 1b 5b 3f 32 30 30 34 24 70 | ESC [ ? 2 0 0 4 $ p | query 2004: set |
| 1b 5b 3f 32 35 24 70 | ESC [ ? 2 5 $ p | query 25: set by default |
| 1b 5b 3f 31 24 70 | ESC [ ? 1 $ p | query 1: reset |
| 1b 5b 34 24 70 | ESC [ 4 $ p | query ANSI 4: reset |
