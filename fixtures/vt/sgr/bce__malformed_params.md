# bce / malformed_params

SGR with an unknown code and a truncated 256-color background before an erase: the red background stays and BCE uses it.

Reference: xterm ctlseqs: SGR, BCE. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 31 3b 39 39 39 3b 34 38 3b 35 6d | ESC [ 4 1 ; 9 9 9 ; 4 8 ; 5 m | red background, unknown 999, truncated 48;5 |
| 1b 5b 32 4b | ESC [ 2 K | EL 2 |
| 78 | x | text |
