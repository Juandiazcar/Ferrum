# osc52 / store

OSC 52 with base64 data reports a clipboard store request (policy is the UI's).

Reference: OSC 52. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 35 32 3b 63 3b 61 47 56 73 62 47 38 3d 07 | ESC ] 5 2 ; c ; a G V s b G 8 = <07> | store 'hello' in the clipboard |
| 1b 5d 35 32 3b 3b 64 32 39 79 62 47 51 3d 07 | ESC ] 5 2 ; ; d 2 9 y b G Q = <07> | default selection |
| 1b 5d 35 32 3b 70 71 3b 07 | ESC ] 5 2 ; p q ; <07> | empty data clears |
