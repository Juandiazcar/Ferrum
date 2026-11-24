# deccolm / basic

DECCOLM clears the screen, homes the cursor and resets margins (the UI owns the size).

Reference: xterm ctlseqs: DECCOLM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 68 65 6c 6c 6f 0d 0a 77 6f 72 6c 64 | h e l l o <0d> <0a> w o r l d | text |
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region |
| 1b 5b 3f 33 68 | ESC [ ? 3 h | DECSET 3 |
| 78 | x | text |
