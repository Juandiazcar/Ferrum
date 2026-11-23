# window_size / malformed_other_operations

Other window operations are ignored.

Reference: xterm ctlseqs: window ops. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 38 3b 35 30 3b 31 30 30 74 | ESC [ 8 ; 5 0 ; 1 0 0 t | resize: ignored |
| 1b 5b 31 34 74 | ESC [ 1 4 t | pixel size: ignored |
| 1b 5b 32 32 3b 30 74 | ESC [ 2 2 ; 0 t | push title: ignored |
| 1b 5b 74 | ESC [ t | no parameter |
