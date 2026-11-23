# osc8 / malformed

A URI with control or non-ASCII bytes, or a missing separator, ends the link instead of starting one.

Reference: docs/10 section 3 item 6. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 38 3b 3b 68 74 74 70 73 3a 2f 2f 65 78 61 6d 70 6c 65 2e 74 65 73 74 2f 07 | ESC ] 8 ; ; h t t p s : / / e x a m p l e . t e s t / <07> | start link |
| 61 | a | linked |
| 1b 5d 38 3b 3b 68 74 74 70 73 3a 2f 2f 65 78 61 01 6d 70 6c 65 2e 74 65 73 74 2f 07 | ESC ] 8 ; ; h t t p s : / / e x a <01> m p l e . t e s t / <07> | control character |
| 62 | b | plain |
| 1b 5d 38 3b 3b 68 74 74 70 73 3a 2f 2f c3 a9 2e 74 65 73 74 2f 07 | ESC ] 8 ; ; h t t p s : / / <c3> <a9> . t e s t / <07> | non-ASCII |
| 63 | c | plain |
| 1b 5d 38 07 | ESC ] 8 <07> | no separator |
| 64 | d | plain |
