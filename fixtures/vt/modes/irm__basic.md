# irm / basic

IRM (CSI 4 h): printing inserts instead of overwriting.

Reference: xterm ctlseqs: IRM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 | a b c d e f | text |
| 1b 5b 31 47 | ESC [ 1 G | CHA 1 |
| 1b 5b 34 68 | ESC [ 4 h | SM 4 |
| 58 59 | X Y | inserted |
| 1b 5b 34 6c | ESC [ 4 l | RM 4 |
| 5a | Z | overwrites |
