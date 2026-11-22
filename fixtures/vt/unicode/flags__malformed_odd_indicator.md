# flags / malformed_odd_indicator

An odd regional indicator stays alone and keeps its own width.

Reference: UAX #29 (RI). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| f0 9f 87 ba | <f0> <9f> <87> <ba> | text |
| 61 | a | text |
| f0 9f 87 ba f0 9f 87 b8 f0 9f 87 a9 | <f0> <9f> <87> <ba> <f0> <9f> <87> <b8> <f0> <9f> <87> <a9> | text |
| 62 | b | text |
