# xtversion / malformed_intermediate

CSI q without the > is DECLL and is ignored.

Reference: xterm ctlseqs: XTVERSION. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 71 | ESC [ q | CSI q: ignored |
| 1b 5b 31 71 | ESC [ 1 q | DECLL: ignored |
