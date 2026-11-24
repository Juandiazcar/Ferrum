# osc8 / id_and_semicolons

The id parameter and a URI containing semicolons are kept; the same link is interned once.

Reference: OSC 8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 38 3b 69 64 3d 78 3a 66 6f 6f 3d 62 61 72 3b 68 74 74 70 73 3a 2f 2f 65 78 61 6d 70 6c 65 2e 74 65 73 74 2f 61 3b 62 07 | ESC ] 8 ; i d = x : f o o = b a r ; h t t p s : / / e x a m p l e . t e s t / a ; b <07> | link with id and a ; in the URI |
| 6f 6e 65 | o n e | linked |
| 1b 5d 38 3b 3b 07 | ESC ] 8 ; ; <07> | end |
| 1b 5d 38 3b 69 64 3d 78 3a 66 6f 6f 3d 62 61 72 3b 68 74 74 70 73 3a 2f 2f 65 78 61 6d 70 6c 65 2e 74 65 73 74 2f 61 3b 62 07 | ESC ] 8 ; i d = x : f o o = b a r ; h t t p s : / / e x a m p l e . t e s t / a ; b <07> | same link again |
| 74 77 6f | t w o | linked |
| 1b 5d 38 3b 3b 07 | ESC ] 8 ; ; <07> | end |
