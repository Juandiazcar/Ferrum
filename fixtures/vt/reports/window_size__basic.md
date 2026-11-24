# window_size / basic

CSI 18 t reports the text area size in characters.

Reference: xterm ctlseqs: window ops. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 38 74 | ESC [ 1 8 t | report text area size |
