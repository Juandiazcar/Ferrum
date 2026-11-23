# bracketed_paste / basic

Bracketed paste (state only; sanitizing is the UI's job).

Reference: xterm ctlseqs: bracketed paste. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 32 30 30 34 68 | ESC [ ? 2 0 0 4 h | DECSET 2004 |
| 61 62 | a b | text |
