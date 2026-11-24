# alt1047 / malformed_interrupted_by_sub

SUB cancels the set before its final byte: the screen stays primary; the h that follows is text.

Reference: xterm ctlseqs: alternate screen, SUB. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 34 37 | ESC [ ? 1 0 4 7 | CSI ? 1 0 4 7 (unfinished) |
| 1a | <1a> | SUB cancels the sequence |
| 68 | h | text |
| 6f 6e 65 | o n e | text on the primary screen |
