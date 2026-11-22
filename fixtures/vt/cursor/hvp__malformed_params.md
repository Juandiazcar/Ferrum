# hvp / malformed_params

HVP clamps like CUP.

Reference: xterm ctlseqs: HVP. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 39 39 3b 30 66 | ESC [ 9 9 ; 0 f | HVP 99;0 |
| 58 | X | marker |
