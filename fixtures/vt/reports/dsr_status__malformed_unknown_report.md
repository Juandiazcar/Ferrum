# dsr_status / malformed_unknown_report

Unknown DSR requests (including title and icon reports) are ignored.

Reference: docs/10 section 3 item 1. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 37 6e | ESC [ 7 n | unknown |
| 1b 5b 32 31 74 | ESC [ 2 1 t | title report: not implemented |
| 1b 5b 32 30 74 | ESC [ 2 0 t | icon report: not implemented |
| 1b 5b 3f 31 35 6e | ESC [ ? 1 5 n | printer status: ignored |
