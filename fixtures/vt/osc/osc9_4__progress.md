# osc9_4 / progress

OSC 9;4 reports progress: 1 normal, 2 error, 3 indeterminate, 4 warning, 0 removes it.

Reference: OSC 9;4. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 39 3b 34 3b 31 3b 32 35 07 | ESC ] 9 ; 4 ; 1 ; 2 5 <07> | normal 25 percent |
| 1b 5d 39 3b 34 3b 32 3b 38 30 07 | ESC ] 9 ; 4 ; 2 ; 8 0 <07> | error 80 percent |
| 1b 5d 39 3b 34 3b 33 07 | ESC ] 9 ; 4 ; 3 <07> | indeterminate |
| 1b 5d 39 3b 34 3b 34 3b 31 30 07 | ESC ] 9 ; 4 ; 4 ; 1 0 <07> | warning |
| 1b 5d 39 3b 34 3b 30 07 | ESC ] 9 ; 4 ; 0 <07> | remove |
