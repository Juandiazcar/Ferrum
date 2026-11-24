# emoji_presentation / malformed_vs16_after_ascii

VS16 after a character that has no emoji presentation does not widen it.

Reference: emoji-variation-sequences. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 ef b8 8f | a <ef> <b8> <8f> | text |
| 62 | b | text |
