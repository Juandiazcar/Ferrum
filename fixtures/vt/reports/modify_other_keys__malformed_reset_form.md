# 1b 5b 3e 34 6d|CSI > 4 m|ignored, bold stays / 78|x|bold text

The reset form CSI > 4 m is ignored too.

Reference: xterm ctlseqs: modifyOtherKeys. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 31 6d | CSI 1 m | bold |
| 1b 5b 3e 34 6d | CSI > 4 m | ignored, bold stays |
| 78 | x | bold text |
