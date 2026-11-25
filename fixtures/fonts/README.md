# fixtures/fonts

Fonts the `ferrum-render` tests install in a temporary directory and scan instead of the system
fonts (`docs/08` §7: tests are hermetic and never read the user's or the runner's own fonts).
Licenses and hashes are in `THIRD_PARTY.md` at the repository root.

| File | Family | Covers | Made from |
|---|---|---|---|
| `FerrumTestGreek.ttf` | Ferrum Test Greek | space, Greek and Coptic (U+0370–U+03FF) | `JetBrainsMono-Regular.ttf` 2.304, renamed |
| `FerrumTestCyrillic.ttf` | Ferrum Test Cyrillic | space, Cyrillic (U+0400–U+04FF) | `JetBrainsMono-Regular.ttf` 2.304, renamed |
| `NotoColorEmoji-subset.ttf` | Noto Color Emoji | 😀 ☕ ✅ 👍 🏽 👨 👩 👧, ZWJ, 🇨 🇴, 1, U+FE0F, U+20E3 and their sequences (`CBDT` color bitmaps) | `NotoColorEmoji.ttf` of noto-emoji `v2026-09-24-unicode18_0` |

Made with fontTools 4.69.1:

```bash
pyftsubset JetBrainsMono-Regular.ttf --unicodes='U+0020,U+0370-03FF' --layout-features='*' \
  --output-file=FerrumTestGreek.ttf        # then name IDs 1, 4, 6, 16 renamed
pyftsubset JetBrainsMono-Regular.ttf --unicodes='U+0020,U+0400-04FF' --layout-features='*' \
  --output-file=FerrumTestCyrillic.ttf     # then name IDs 1, 4, 6, 16 renamed
pyftsubset NotoColorEmoji.ttf --layout-features='*' --output-file=NotoColorEmoji-subset.ttf \
  --unicodes='U+1F600,U+2615,U+2705,U+1F44D,U+1F3FD,U+1F468,U+1F469,U+1F467,U+200D,U+1F1E8,U+1F1F4,U+FE0F,U+0031,U+20E3'
```
