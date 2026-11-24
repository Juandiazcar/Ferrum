# emoji_zwj / malformed_dangling_joiner

A ZWJ at the end of the text or before ASCII joins nothing beyond the previous cell.

Reference: UAX #29. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 e2 80 8d | a <e2> <80> <8d> | text |
| 62 | b | text |
| f0 9f 91 a8 e2 80 8d | <f0> <9f> <91> <a8> <e2> <80> <8d> | text |
| 63 | c | text |
