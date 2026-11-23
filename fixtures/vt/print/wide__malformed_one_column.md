# wide / malformed_one_column

On a one-column terminal a wide character cannot be shown and is dropped.

Reference: xterm: wide characters. Terminal size: 1x3.

| hex | bytes | meaning |
|---|---|---|
| e6 bc a2 61 | <e6> <bc> <a2> a | wide then ASCII |
