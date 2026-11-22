# wide / wraps_at_edge

A wide character that does not fit in the last column leaves a leading spacer (shown as the guillemet) and wraps.

Reference: xterm: wide characters. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 | a a a a a a a a a a a a a a a a a a a | 19 columns used |
| e6 bc a2 | <e6> <bc> <a2> | wide, needs 2 columns |
| 7a | z | text |
