# Third-party content

Code, fonts and data that Ferrum ships or tests with and did not write (`docs/10` §6). Rust
dependencies are tracked by `Cargo.lock`, `cargo deny` and `docs/spec/docs/07-tech-stack-and-versions.md`.

## Fonts

| File | What | License | Source | sha256 |
|---|---|---|---|---|
| `assets/fonts/JetBrainsMono-wght-subset.ttf` | JetBrains Mono 2.304 variable, subset (embedded default font) | SIL OFL 1.1 (`assets/fonts/OFL.txt`) | `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`, `fonts/variable/JetBrainsMono[wght].ttf` (sha256 `662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016`) | `901eebf4aef4c5dff67831227fa6dd3fadcf164fa75ddc2677d39d89db6fa463` |
| `fixtures/fonts/FerrumTestGreek.ttf` | JetBrains Mono 2.304 Regular, Greek subset, renamed (tests only) | SIL OFL 1.1 (`fixtures/fonts/OFL-JetBrainsMono.txt`) | same zip, `fonts/ttf/JetBrainsMono-Regular.ttf` (sha256 `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`) | `c1bda9413733e154c3e150fec96826495973629c3b0332745afadb9bd982f720` |
| `fixtures/fonts/FerrumTestCyrillic.ttf` | JetBrains Mono 2.304 Regular, Cyrillic subset, renamed (tests only) | SIL OFL 1.1 (`fixtures/fonts/OFL-JetBrainsMono.txt`) | same file | `320a0f0be1a342b6a5ad96ce360a282b1176594e7b660b77d21cb7032da4f9e0` |
| `fixtures/fonts/NotoColorEmoji-subset.ttf` | Noto Color Emoji, subset (tests only) | SIL OFL 1.1 (`fixtures/fonts/OFL-NotoColorEmoji.txt`) | `https://raw.githubusercontent.com/googlefonts/noto-emoji/v2026-09-24-unicode18_0/2D/fonts/NotoColorEmoji.ttf` (sha256 `15671215ab769fdc7162a045d56fd7d7e477c51b04e6b3c761d914d8fdd6cc44`) | `cd257420b8f1bd348384dc855e3b5ae271fa6ddbe55b4a52905c34a02628361c` |

The subset commands are in `assets/fonts/README.md` and `fixtures/fonts/README.md`.

## Test data

- `fixtures/vt/`: Alacritty reference tests, see `fixtures/vt/THIRD_PARTY.md`.
- `fixtures/unicode/`: Unicode data files, Unicode License v3 (`fixtures/unicode/LICENSE-UNICODE.txt`).
