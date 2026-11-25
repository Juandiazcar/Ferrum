# assets/fonts

Embedded default monospace font (`docs/07` §5, `docs/09` §9), compiled into `ferrum-render`.

| File | What |
|---|---|
| `JetBrainsMono-wght-subset.ttf` | JetBrains Mono 2.304 variable (`wght` axis 100–800), subset |
| `OFL.txt` | Its license, SIL Open Font License 1.1 (no Reserved Font Name) |

Source: `JetBrainsMono[wght].ttf` from the latest release, v2.304
(`https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`,
checked 2026-11-25), sha256 `662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016`
(303 144 bytes, the same file as spike S3).

Subset (162 964 bytes, sha256 `901eebf4aef4c5dff67831227fa6dd3fadcf164fa75ddc2677d39d89db6fa463`)
made with fontTools 4.69.1, keeping the variation tables, the hinting program and every layout
feature:

```bash
pyftsubset 'JetBrainsMono[wght].ttf' \
  --unicodes='U+0020-007E,U+00A0-024F,U+0300-036F,U+1E00-1EFF,U+2500-259F,U+E0A0-E0A3,U+E0B0-E0B3' \
  --layout-features='*' --notdef-outline --name-IDs='*' --name-languages='*' \
  --output-file=JetBrainsMono-wght-subset.ttf
```

Ranges: Basic Latin, Latin-1 Supplement, Latin Extended-A and -B, the combining diacritical marks
Latin text uses, Latin Extended Additional, box drawing, block elements and the powerline symbols.
Box drawing, block elements and the powerline separators are drawn by `ferrum-render` itself
(pixel-perfect, bypassing the font); their outlines stay in the file as `docs/07` §5 lists them.
Every other character comes from the system fonts.
