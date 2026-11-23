# wide / malformed_overwrite_half

Overwriting one half of a wide character blanks the other half.

Reference: xterm: wide characters. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| e6 bc a2 | <e6> <bc> <a2> | wide at columns 0-1 |
| 1b 5b 44 | ESC [ D | CUB 1: cursor on the spacer |
| 58 | X | overwrites the spacer |
