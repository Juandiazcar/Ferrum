# decrqss / malformed_unknown_and_long

Unknown settings, over-long requests and an unterminated string get the invalid reply or nothing.

Reference: docs/10 section 3 item 3. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 50 24 71 7a 7a 7a 1b 5c | ESC P $ q z z z ESC \ | unknown setting |
| 1b 50 24 71 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 6d 1b 5c | ESC P $ q m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m m ESC \ | 100-byte request |
| 1b 50 24 71 1b 5c | ESC P $ q ESC \ | empty request |
| 1b 50 24 71 6d | ESC P $ q m | unterminated: no reply yet |
