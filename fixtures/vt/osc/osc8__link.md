# osc8 / link

OSC 8 links the text between the start and the end sequence.

Reference: OSC 8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | plain |
| 1b 5d 38 3b 3b 68 74 74 70 73 3a 2f 2f 65 78 61 6d 70 6c 65 2e 74 65 73 74 2f 07 | ESC ] 8 ; ; h t t p s : / / e x a m p l e . t e s t / <07> | start link |
| 6c 69 6e 6b | l i n k | linked text |
| 1b 5d 38 3b 3b 07 | ESC ] 8 ; ; <07> | end link |
| 62 | b | plain |
