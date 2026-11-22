# apc_pm_sos / ignored

APC, PM and SOS strings are discarded without storing them.

Reference: docs/10 section 3 item 9. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5f 47 69 3d 31 3b 70 61 79 6c 6f 61 64 1b 5c | ESC _ G i = 1 ; p a y l o a d ESC \ | APC |
| 1b 5e 70 72 69 76 61 74 65 1b 5c | ESC ^ p r i v a t e ESC \ | PM |
| 1b 58 73 74 72 69 6e 67 1b 5c | ESC X s t r i n g ESC \ | SOS |
| 61 | a | text |
