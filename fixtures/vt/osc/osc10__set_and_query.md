# osc10 / set_and_query

OSC 10 sets or queries the default foreground; extra arguments address the next colors.

Reference: xterm ctlseqs: OSC 10. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 30 3b 3f 07 | ESC ] 1 0 ; ? <07> | query foreground |
| 1b 5d 31 30 3b 23 31 31 32 32 33 33 07 | ESC ] 1 0 ; # 1 1 2 2 3 3 <07> | set foreground |
| 1b 5d 31 30 3b 3f 07 | ESC ] 1 0 ; ? <07> | query again |
| 1b 5d 31 30 3b 23 30 31 30 31 30 31 3b 23 30 32 30 32 30 32 07 | ESC ] 1 0 ; # 0 1 0 1 0 1 ; # 0 2 0 2 0 2 <07> | set foreground and background |
| 1b 5d 31 31 3b 3f 07 | ESC ] 1 1 ; ? <07> | query background |
