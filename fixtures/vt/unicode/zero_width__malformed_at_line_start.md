# zero_width / malformed_at_line_start

Zero-width characters with no cell before them are dropped.

Reference: UAX #11. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| e2 80 8b e2 80 8d cc 81 | <e2> <80> <8b> <e2> <80> <8d> <cc> <81> | text |
| 61 | a | text |
