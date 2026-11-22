# width / malformed_c1_and_noncharacters

C1 controls are executed; noncharacters print as one column.

Reference: UAX #11. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| c3 a9 c2 85 c2 9b | <c3> <a9> <c2> <85> <c2> <9b> | e acute, NEL, CSI as scalars |
| 61 | a | text |
| ef b7 90 | <ef> <b7> <90> | noncharacter |
| 62 | b | text |
