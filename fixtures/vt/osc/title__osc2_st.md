# title / osc2_st

OSC 2 terminated by ST (ESC \) sets the window title.

Reference: xterm ctlseqs: OSC 2. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 32 3b 73 65 63 6f 6e 64 1b 5c | ESC ] 2 ; s e c o n d ESC \ | OSC 2 ; text ST |
| 61 | a | text |
