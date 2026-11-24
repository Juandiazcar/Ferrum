# irm / malformed_unknown_ansi_mode

Unknown ANSI modes are ignored.

Reference: xterm ctlseqs: SM. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 39 39 39 68 | ESC [ 9 9 9 h | unknown |
| 1b 5b 32 3b 34 68 | ESC [ 2 ; 4 h | KAM (ignored) and IRM |
| 61 | a | text |
