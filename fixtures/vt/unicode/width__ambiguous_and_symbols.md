# width / ambiguous_and_symbols

Ambiguous-width characters are narrow (no CJK context); box drawing and math symbols are one column.

Reference: UAX #11 (A is narrow). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| c2 b1 c3 97 c3 b7 e2 94 80 e2 94 82 e2 96 88 | <c2> <b1> <c3> <97> <c3> <b7> <e2> <94> <80> <e2> <94> <82> <e2> <96> <88> | text |
| ce b1 ce b2 ce b3 | <ce> <b1> <ce> <b2> <ce> <b3> | text |
| e2 86 92 | <e2> <86> <92> | text |
