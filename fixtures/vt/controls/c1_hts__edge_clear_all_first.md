# c1_hts / edge_clear_all_first

HTS after clearing every stop leaves a single stop.

Reference: xterm ctlseqs: HTS (C1). Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 33 67 | CSI 3 g | clear all stops |
| 09 | HT | to the last column |
| 88 | <88> | HTS on the last column |
| 0d 78 09 09 79 | CR x HT HT y | tabs stop at the one stop and then at the last column |
