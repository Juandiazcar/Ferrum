# decawm / basic

DECAWM off: text at the right margin overwrites the last column instead of wrapping.

Reference: xterm ctlseqs: DECAWM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 37 6c | ESC [ ? 7 l | DECRST 7 |
| 61 62 63 64 65 66 67 68 69 6a 6b 6c 6d 6e 6f 70 71 72 73 74 75 76 77 78 79 7a | a b c d e f g h i j k l m n o p q r s t u v w x y z | 26 characters in 20 columns |
