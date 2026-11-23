# capture / malformed_unterminated_then_esc

An OSC cut off by another escape sequence is still dispatched, then the sequence runs.

Reference: xterm ctlseqs: OSC. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 32 3b 63 75 74 | ESC ] 2 ; c u t | OSC without terminator |
| 1b 5b 32 3b 33 48 | ESC [ 2 ; 3 H | CUP 2;3 (ESC ends the OSC) |
| 78 | x | text |
