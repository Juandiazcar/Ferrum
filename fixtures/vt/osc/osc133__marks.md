# osc133 / marks

OSC 133 marks the prompt, the command start, the output start and the command end on the cursor row.

Reference: OSC 133. Terminal size: 20x6.

| hex | bytes | meaning |
|---|---|---|
| 1b 5d 31 33 33 3b 41 07 | ESC ] 1 3 3 ; A <07> | prompt start |
| 24 20 | $ SP | prompt |
| 1b 5d 31 33 33 3b 42 07 | ESC ] 1 3 3 ; B <07> | command start |
| 6c 73 0d 0a | l s <0d> <0a> | command |
| 1b 5d 31 33 33 3b 43 07 | ESC ] 1 3 3 ; C <07> | output start |
| 66 69 6c 65 0d 0a | f i l e <0d> <0a> | output |
| 1b 5d 31 33 33 3b 44 3b 30 07 | ESC ] 1 3 3 ; D ; 0 <07> | command end, exit 0 |
| 1b 5d 31 33 33 3b 41 3b 6b 3d 69 07 | ESC ] 1 3 3 ; A ; k = i <07> | next prompt with options |
| 24 20 | $ SP | prompt |
