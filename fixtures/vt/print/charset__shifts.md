# charset / shifts

ESC n / ESC o lock G2 / G3 into GL; ESC N / ESC O shift one character.

Reference: xterm ctlseqs: LS2, LS3, SS2, SS3. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 2a 30 | ESC * 0 | G2 = line drawing |
| 1b 4e | ESC N | SS2 |
| 71 71 | q q | first is line drawing, second plain |
| 1b 2b 30 | ESC + 0 | G3 = line drawing |
| 1b 6f | ESC o | LS3 |
| 71 71 | q q | both line drawing |
| 1b 6e | ESC n | LS2 |
| 71 | q | line drawing from G2 |
