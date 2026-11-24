# title / malformed_control_chars

Control characters are stripped from titles (docs/10 section 3 item 1).

Reference: docs/10 section 3 item 1. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 30 3b 61 01 62 7f 63 c2 85 64 07 | ESC ] 0 ; a <01> b <7f> c <c2> <85> d <07> | title with SOH, DEL and a C1 control |
| 61 | a | text |
