# da2 / malformed_intermediate

CSI = c (DA3) and CSI ? c are not answered.

Reference: xterm ctlseqs: Secondary DA. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3d 63 | ESC [ = c | DA3: ignored |
| 1b 5b 3f 63 | ESC [ ? c | private DA: ignored |
| 61 | a | text |
