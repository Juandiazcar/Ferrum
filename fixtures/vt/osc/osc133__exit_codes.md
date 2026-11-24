# osc133 / exit_codes

The exit code of D is optional and may be negative.

Reference: OSC 133. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 33 33 3b 44 3b 31 33 30 07 | ESC ] 1 3 3 ; D ; 1 3 0 <07> | exit 130 |
| 1b 5d 31 33 33 3b 44 3b 2d 31 07 | ESC ] 1 3 3 ; D ; - 1 <07> | exit -1 |
| 1b 5d 31 33 33 3b 44 07 | ESC ] 1 3 3 ; D <07> | no exit code |
| 1b 5d 31 33 33 3b 44 3b 61 62 63 07 | ESC ] 1 3 3 ; D ; a b c <07> | invalid exit code |
