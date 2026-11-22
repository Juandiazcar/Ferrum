# alt1049 / no_history

Rows scrolling off the alternate screen are not kept in the scrollback.

Reference: xterm ctlseqs: mode 1049. Terminal size: 20x3.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 3f 31 30 34 39 68 | ESC [ ? 1 0 4 9 h | enter |
| 31 0d 0a 32 0d 0a 33 0d 0a 34 0d 0a 35 | 1 <0d> <0a> 2 <0d> <0a> 3 <0d> <0a> 4 <0d> <0a> 5 | five rows on a three-row screen |
| 1b 5b 3f 31 30 34 39 6c | ESC [ ? 1 0 4 9 l | leave |
