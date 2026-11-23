# unknown / ignored

Unknown DCS strings are discarded; text after them prints normally.

Reference: xterm ctlseqs: DCS. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 50 71 23 30 3b 32 3b 30 3b 30 3b 30 23 30 7e 7e 7e 1b 5c | ESC P q # 0 ; 2 ; 0 ; 0 ; 0 # 0 ~ ~ ~ ESC \ | a sixel string |
| 61 | a | text |
| 1b 50 31 3b 32 7c 61 62 63 1b 5c | ESC P 1 ; 2 \| a b c ESC \ | user-defined keys |
| 62 | b | text |
