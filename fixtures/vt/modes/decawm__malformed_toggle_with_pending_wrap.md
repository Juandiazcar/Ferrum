# decawm / malformed_toggle_with_pending_wrap

Turning auto-wrap off while a wrap is pending drops the pending wrap.

Reference: xterm ctlseqs: DECAWM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 67 68 69 6a 6b 6c 6d 6e 6f 70 71 72 73 74 | a b c d e f g h i j k l m n o p q r s t | fills the row: wrap pending |
| 1b 5b 3f 37 6c | ESC [ ? 7 l | DECRST 7 |
| 58 59 | X Y | overwrite the last column |
| 1b 5b 3f 37 68 | ESC [ ? 7 h | DECSET 7 |
| 5a | Z | wraps now |
