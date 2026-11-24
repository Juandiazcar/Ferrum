# 0d|CR|cursor home / 62|b|overwrites, no join

After a carriage return the next scalar starts a new cluster.

Reference: UAX #29 GB9b. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| d8 80 | U+0600 | prepend scalar |
| 0d | CR | cursor home |
| 62 | b | overwrites, no join |
