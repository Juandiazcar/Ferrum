# 61 62|a b|curly underlined, not double / 

SGR 21 after a curly underline in the same sequence leaves the underline alone.

Reference: Alacritty reference test underline. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 3a 33 3b 32 31 6d | ESC [ 4:3 ; 2 1 m | curly underline, then SGR 21 |
| 61 62 | a b | curly underlined, not double |
