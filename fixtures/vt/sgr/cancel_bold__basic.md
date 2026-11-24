# 61|a|bold underlined / 1b 5b 32 31 6d|ESC [ 2 1 m|SGR 21: bold off, underline stays

SGR 21 cancels bold (Alacritty reference behavior).

Reference: Alacritty reference test underline. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 3b 34 6d | ESC [ 1 ; 4 m | bold and underline |
| 61 | a | bold underlined |
| 1b 5b 32 31 6d | ESC [ 2 1 m | SGR 21: bold off, underline stays |
| 62 | b | underlined only |
| 1b 5b 30 6d 63 | ESC [ 0 m c | plain |
