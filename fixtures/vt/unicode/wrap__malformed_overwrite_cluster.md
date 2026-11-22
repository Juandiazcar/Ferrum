# wrap / malformed_overwrite_cluster

Overwriting part of a wide cluster blanks all of it.

Reference: xterm: wide characters. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| f0 9f 91 a8 e2 80 8d f0 9f 91 a9 | <f0> <9f> <91> <a8> <e2> <80> <8d> <f0> <9f> <91> <a9> | text |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 78 | x | text |
