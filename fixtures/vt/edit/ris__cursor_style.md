# ris / cursor_style

RIS resets the cursor style set by DECSCUSR (Alacritty `Term::reset_state` sets `cursor_style =
None`; maintainer decisions Q-23 and Q-25). DECRQSS for the cursor style then reports the default
(steady block, 2).

Reference: xterm ctlseqs: RIS, DECSCUSR, DECRQSS; Alacritty `alacritty_terminal` 0.26.0
`Term::reset_state`.

| hex | bytes | meaning |
|---|---|---|
| 1b 5b 34 20 71 | ESC [ 4 SP q | DECSCUSR 4: steady underline |
| 1b 63 | ESC c | RIS |
| 1b 50 24 71 20 71 1b 5c | ESC P $ q SP q ESC \ | DECRQSS: cursor style |
