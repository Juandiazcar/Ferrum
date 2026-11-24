# wrap / wide_cluster_at_margin

A wide cluster that does not fit in the last column wraps whole, leaving a leading spacer.

Reference: xterm: wide characters. Terminal size: 20x4.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 | a a a a a a a a a a a a a a a a a a a | text |
| f0 9f 91 a8 e2 80 8d f0 9f 91 a9 e2 80 8d f0 9f 91 a7 | <f0> <9f> <91> <a8> <e2> <80> <8d> <f0> <9f> <91> <a9> <e2> <80> <8d> <f0> <9f> <91> <a7> | text |
| 7a | z | text |
