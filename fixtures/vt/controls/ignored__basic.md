# ignored / basic

NUL, ENQ (no answerback) and other unhandled C0 controls print nothing and reply nothing.

Reference: docs/10 section 3 item 3. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 | a | text |
| 00 05 01 1c | <00> <05> <01> <1c> | NUL ENQ SOH FS |
| 62 | b | text |
