# osc133 / malformed

Unknown marks and an empty OSC 133 are ignored.

Reference: OSC 133. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 33 33 07 | ESC ] 1 3 3 <07> | no argument |
| 1b 5d 31 33 33 3b 58 07 | ESC ] 1 3 3 ; X <07> | unknown mark |
| 1b 5d 31 33 33 3b 07 | ESC ] 1 3 3 ; <07> | empty mark |
| 1b 5d 31 33 33 3b 41 42 07 | ESC ] 1 3 3 ; A B <07> | two letters |
| 61 | a | text |
