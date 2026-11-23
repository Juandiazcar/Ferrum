# history / wrapped_rows

A soft-wrapped row keeps its wrapped flag when it scrolls into history.

Reference: docs/02 section 3.2. Terminal size: 10x3.

| hex | bytes | meaning |
|---|---|---|
| 61 62 63 64 65 66 67 68 69 6a 6b 6c 6d 6e 6f 70 71 72 73 74 75 76 77 78 79 7a 0d 0a 6e 65 78 74 0d 0a 6d 6f 72 65 0d 0a 6c 61 73 74 | a b c d e f g h i j k l m n o p q r s t u v w x y z <0d> <0a> n e x t <0d> <0a> m o r e <0d> <0a> l a s t | one wrapped logical line then three rows |
