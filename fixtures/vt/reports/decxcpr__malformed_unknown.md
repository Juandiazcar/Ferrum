# decxcpr / malformed_unknown

Other private DSR requests are ignored.

Reference: xterm ctlseqs: DECXCPR. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 39 6e | ESC [ ? 9 n | ignored |
| 1b 5b 3f 36 3b 37 6e | ESC [ ? 6 ; 7 n | extra parameter |
