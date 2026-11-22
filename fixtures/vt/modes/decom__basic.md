# decom / basic

DECOM: CUP is relative to the scroll region and the cursor homes to its top.

Reference: xterm ctlseqs: DECOM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 32 3b 34 72 | ESC [ 2 ; 4 r | region 2-4 |
| 1b 5b 3f 36 68 | ESC [ ? 6 h | DECSET 6 |
| 1b 5b 31 3b 31 48 | ESC [ 1 ; 1 H | CUP 1;1 = region top |
| 41 | A | marker |
| 1b 5b 39 3b 31 48 | ESC [ 9 ; 1 H | CUP 9;1 clamped to the region bottom |
| 42 | B | marker |
