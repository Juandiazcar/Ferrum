# osc4 / malformed_specs

Invalid indices and color specs are ignored; valid pairs around them still apply.

Reference: xterm ctlseqs: OSC 4. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 34 3b 39 39 39 3b 23 66 66 66 66 66 66 3b 31 3b 6e 6f 74 61 63 6f 6c 6f 72 3b 78 3b 23 66 66 66 66 66 66 3b 32 3b 23 30 30 30 30 66 66 3b 33 07 | ESC ] 4 ; 9 9 9 ; # f f f f f f ; 1 ; n o t a c o l o r ; x ; # f f f f f f ; 2 ; # 0 0 0 0 f f ; 3 <07> | bad index, bad spec, non-numeric index, good pair, dangling index |
| 61 | a | text |
