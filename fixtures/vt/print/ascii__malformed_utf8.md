# ascii / malformed_utf8

Invalid UTF-8 prints U+FFFD per bad byte and keeps going.

Reference: UTF-8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 ff 62 c3 28 63 | a <ff> b <c3> ( c | a, 0xFF, b, truncated 2-byte lead, (, c |
