# emoji / malformed_truncated

A truncated 4-byte emoji prints U+FFFD and the next character prints normally.

Reference: UTF-8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| f0 9f 98 | <f0> <9f> <98> | three bytes of a 4-byte emoji |
| 61 | a | text |
