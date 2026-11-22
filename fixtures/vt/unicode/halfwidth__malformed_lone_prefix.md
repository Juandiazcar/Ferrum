# halfwidth / malformed_lone_prefix

A lone leading byte before ASCII prints U+FFFD, then the ASCII.

Reference: UTF-8. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| ef | <ef> | lone lead byte |
| 61 62 | a b | text |
