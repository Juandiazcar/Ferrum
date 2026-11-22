# cjk / malformed_truncated

A truncated UTF-8 sequence inside CJK text prints U+FFFD and the text resumes.

Reference: UTF-8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| e6 bc | <e6> <bc> | first two bytes of a 3-byte character |
| 61 62 | a b | text |
| e6 bc a2 | <e6> <bc> <a2> | complete |
| 63 | c | text |
