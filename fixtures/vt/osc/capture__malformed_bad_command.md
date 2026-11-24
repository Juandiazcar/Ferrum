# capture / malformed_bad_command

A non-numeric or oversized OSC command is ignored.

Reference: xterm ctlseqs: OSC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 78 3b 6e 6f 74 68 69 6e 67 07 | ESC ] x ; n o t h i n g <07> | non-numeric |
| 1b 5d 39 39 39 39 39 39 3b 6e 6f 74 68 69 6e 67 07 | ESC ] 9 9 9 9 9 9 ; n o t h i n g <07> | too many digits |
| 1b 5d 3b 6e 6f 74 68 69 6e 67 07 | ESC ] ; n o t h i n g <07> | empty command |
| 61 | a | text |
