# alt47 / malformed_interrupted_by_can

CAN cancels the set before its final byte: the screen stays primary; the h that follows is text.

Reference: xterm ctlseqs: alternate screen, CAN. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 34 37 | ESC [ ? 4 7 | CSI ? 4 7 (unfinished) |
| 18 | <18> | CAN cancels the sequence |
| 68 | h | text |
| 6f 6e 65 | o n e | text on the primary screen |
