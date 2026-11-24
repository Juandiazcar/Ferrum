# so_si / basic

SO selects G1 (line drawing here), SI selects G0 again.

Reference: xterm ctlseqs: SO, SI. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 29 30 | ESC ) 0 | G1 = DEC special graphics |
| 0e | <0e> | SO |
| 71 78 | q x | line drawing |
| 0f | <0f> | SI |
| 71 78 | q x | ASCII |
