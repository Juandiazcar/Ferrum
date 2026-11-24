# alt1049 / malformed_junk_params

1049 with empty and huge extra parameters: 1049 still applies, the junk is ignored.

Reference: xterm ctlseqs: alternate screen. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 6f 6e 65 | o n e | text on the primary screen |
| 1b 5b 3f 31 30 34 39 3b 3b 39 39 39 39 39 39 39 39 39 39 39 68 | ESC [ ? 1 0 4 9 ; ; 9 9 9 9 9 9 9 9 9 9 9 h | 1049 with junk parameters |
| 74 77 6f | t w o | text |
