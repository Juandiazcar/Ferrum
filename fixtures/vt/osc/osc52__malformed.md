# osc52 / malformed

Invalid base64, bad selection letters and a missing separator are ignored.

Reference: docs/10 section 3 item 4. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 35 32 3b 63 3b 21 21 21 6e 6f 74 20 62 61 73 65 36 34 07 | ESC ] 5 2 ; c ; ! ! ! n o t SP b a s e 6 4 <07> | invalid base64 |
| 1b 5d 35 32 3b 7a 7a 3b 61 47 56 73 62 47 38 3d 07 | ESC ] 5 2 ; z z ; a G V s b G 8 = <07> | invalid selection |
| 1b 5d 35 32 3b 63 07 | ESC ] 5 2 ; c <07> | no data part |
| 1b 5d 35 32 07 | ESC ] 5 2 <07> | no arguments |
| 1b 5d 35 32 3b 63 3b 61 47 56 73 62 47 38 3d 07 | ESC ] 5 2 ; c ; a G V s b G 8 = <07> | valid |
