# combining / malformed_after_wide_and_at_margin

Marks after a wide character join it; a mark at the start of a row after a wrap joins the previous cell.

Reference: UAX #29. Terminal size: 20x4.

| hex | bytes | meaning |
|---|---|---|
| e6 bc a2 cc 81 | <e6> <bc> <a2> <cc> <81> | text |
| 61 62 63 64 65 66 67 68 69 6a 6b 6c 6d 6e 6f 70 71 72 73 | a b c d e f g h i j k l m n o p q r s | text |
| 74 | t | text |
| cc 81 | <cc> <81> | text |
| 75 | u | text |
