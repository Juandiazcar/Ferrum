# osc52 / query

OSC 52 with ? reports a clipboard read request; the terminal never answers by itself.

Reference: OSC 52. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 35 32 3b 63 3b 3f 07 | ESC ] 5 2 ; c ; ? <07> | read the clipboard |
| 1b 5d 35 32 3b 3b 3f 07 | ESC ] 5 2 ; ; ? <07> | default selection |
