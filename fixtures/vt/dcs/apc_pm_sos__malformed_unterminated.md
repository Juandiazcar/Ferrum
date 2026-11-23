# apc_pm_sos / malformed_unterminated

An unterminated APC swallows input until a terminator, then recovers.

Reference: docs/10 section 3 item 9. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5f 6e 65 76 65 72 20 65 6e 64 73 | ESC _ n e v e r SP e n d s | unterminated APC |
| 1b 5c | ESC \ | ST |
| 61 | a | text |
