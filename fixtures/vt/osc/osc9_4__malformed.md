# osc9_4 / malformed

A value above 100 is clamped; an unknown state and other OSC 9 forms are ignored.

Reference: OSC 9;4. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 39 3b 34 3b 31 3b 32 35 30 07 | ESC ] 9 ; 4 ; 1 ; 2 5 0 <07> | clamped to 100 |
| 1b 5d 39 3b 34 3b 39 3b 31 30 07 | ESC ] 9 ; 4 ; 9 ; 1 0 <07> | unknown state |
| 1b 5d 39 3b 68 65 6c 6c 6f 07 | ESC ] 9 ; h e l l o <07> | a notification |
| 1b 5d 39 3b 34 07 | ESC ] 9 ; 4 <07> | no state |
| 1b 5d 39 3b 34 3b 31 07 | ESC ] 9 ; 4 ; 1 <07> | no value |
