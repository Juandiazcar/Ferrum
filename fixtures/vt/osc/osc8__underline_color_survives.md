# osc8 / underline_color_survives

A hyperlink and an underline color share one style record per combination.

Reference: OSC 8, SGR 58. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 3b 35 38 3b 32 3b 32 35 35 3b 30 3b 30 6d | ESC [ 4 ; 5 8 ; 2 ; 2 5 5 ; 0 ; 0 m | red underline |
| 1b 5d 38 3b 3b 68 74 74 70 73 3a 2f 2f 65 78 61 6d 70 6c 65 2e 74 65 73 74 2f 07 | ESC ] 8 ; ; h t t p s : / / e x a m p l e . t e s t / <07> | link |
| 61 62 | a b | text |
| 1b 5d 38 3b 3b 07 | ESC ] 8 ; ; <07> | end link |
| 63 64 | c d | text |
