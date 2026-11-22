# osc12 / set_and_query

OSC 12 sets or queries the cursor color.

Reference: xterm ctlseqs: OSC 12. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 32 3b 23 61 62 63 64 65 66 07 | ESC ] 1 2 ; # a b c d e f <07> | set cursor color |
| 1b 5d 31 32 3b 3f 07 | ESC ] 1 2 ; ? <07> | query cursor color |
