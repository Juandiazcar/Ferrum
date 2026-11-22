# rep / malformed_without_character

REP with nothing printed before does nothing; no parameter means 1.

Reference: xterm ctlseqs: REP. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 62 | ESC [ 3 b | REP 3 (nothing yet) |
| 78 | x | text |
| 1b 5b 62 | ESC [ b | REP with no parameter = 1 |
