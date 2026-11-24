//! Settings → Appearance (`docs/09` §8, `docs/09` §9).
//!
//! Every default and bound below is either fixed by the spec (cited) or chosen and recorded in
//! `docs/14` Q-30 with the reason given here; all of them are editable in the settings UI.

use serde::{Deserialize, Serialize};

use crate::value::{Rgb, bounded};

/// Name of the default theme (`docs/09` §9).
pub const DEFAULT_THEME: &str = "Ferrum Dark";

/// Default font family: the embedded variable font (`docs/09` §9, `docs/07` §5).
pub const DEFAULT_FONT_FAMILY: &str = "JetBrains Mono";

bounded!(
    /// Terminal font size in points. Default 13: the 13 px step of the `docs/09` §9 type scale at
    /// UI scale 100 %. Bounds: below 6 pt glyphs are not legible at scale 1.0; above 72 pt a
    /// 1440×900 reference window holds fewer than the 3 rows of the minimum pane (`docs/09` §4).
    FontSize(f32),
    min = 6.0,
    max = 72.0,
    default = 13.0
);
bounded!(
    /// Line height as a multiple of the font's natural height. Default 1.0: the font's own
    /// metrics, no extra rows lost. Bounds: below 0.8 ascenders and descenders of adjacent rows
    /// overlap; above 2.0 half of every row is empty.
    LineHeight(f32),
    min = 0.8,
    max = 2.0,
    default = 1.0
);
bounded!(
    /// Extra space between cells in logical pixels. Default 0: the font's own advance. Bounds:
    /// −2 px keeps cells of a 6 pt font from overlapping; 10 px is wider than a whole cell of the
    /// default size, past any useful spacing.
    LetterSpacing(f32),
    min = -2.0,
    max = 10.0,
    default = 0.0
);
bounded!(
    /// Horizontal window padding in logical pixels (`docs/09` §5: default 8 px). Bound: 64 px,
    /// 16 steps of the 4 px spacing scale (`docs/09` §9); more only hides terminal columns.
    PaddingX(u16),
    min = 0,
    max = 64,
    default = 8
);
bounded!(
    /// Vertical window padding in logical pixels (`docs/09` §5: default 6 px). Bound: as
    /// [`PaddingX`].
    PaddingY(u16),
    min = 0,
    max = 64,
    default = 6
);
bounded!(
    /// Sidebar width in logical pixels (`docs/09` §1: default 248, resizable 160–420).
    SidebarWidth(u16),
    min = 160,
    max = 420,
    default = 248
);
bounded!(
    /// UI scale in percent (`docs/09` §11: 75–200 %; default 100 %, the 1× reference).
    UiScale(u16),
    min = 75,
    max = 200,
    default = 100
);

/// Cursor shape (`FR-TERM-017`). Default block: the xterm default shape that `DECSCUSR 0`
/// restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CursorStyle {
    /// Full cell.
    #[default]
    Block,
    /// Line under the cell.
    Underline,
    /// Vertical bar before the cell.
    Bar,
}

/// Sidebar row density. Default comfortable: the spacing of the `docs/09` §1 mockup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Density {
    /// Default spacing.
    #[default]
    Comfortable,
    /// Tighter rows.
    Compact,
}

/// Appearance section.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent on/off setting shown as a toggle in the UI (docs/09 §8)"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Appearance {
    /// Theme name (built-in or user theme file).
    pub theme: String,
    /// Font family name.
    pub font_family: String,
    /// Font size.
    pub font_size: FontSize,
    /// Line height.
    pub line_height: LineHeight,
    /// Letter spacing.
    pub letter_spacing: LetterSpacing,
    /// Draw bold text in the bright palette colors. Default off: bold stays bold in its own color,
    /// so truecolor and 256-color programs render as written.
    pub bold_is_bright: bool,
    /// Font families with ligatures on (`FR-TERM-019`: a toggle per font, off by default, so the
    /// default list is empty).
    pub ligature_fonts: Vec<String>,
    /// Cursor shape.
    pub cursor_style: CursorStyle,
    /// Blink the cursor while the window is focused (`FR-TERM-017`). Default on: the conventional
    /// terminal default; it costs nothing when unfocused or idle (`NFR-PERF-06`).
    pub cursor_blink: bool,
    /// Cursor color; `None` uses the theme's cursor color (themes own colors, `docs/09` §9).
    pub cursor_color: Option<Rgb>,
    /// Horizontal padding.
    pub padding_x: PaddingX,
    /// Vertical padding.
    pub padding_y: PaddingY,
    /// Sidebar width.
    pub sidebar_width: SidebarWidth,
    /// Sidebar density.
    pub sidebar_density: Density,
    /// UI scale.
    pub ui_scale: UiScale,
    /// Tint the pane background with the project color (`docs/09` §5: default off).
    pub project_tint: bool,
    /// UI transitions (`docs/09` §10: optional, at most 120 ms). Default on: they are short and
    /// never continuous; reduced motion turns them off.
    pub animations: bool,
    /// Reduced motion (`docs/09` §11). Default off: the user opts in.
    pub reduced_motion: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_owned(),
            font_family: DEFAULT_FONT_FAMILY.to_owned(),
            font_size: FontSize::default(),
            line_height: LineHeight::default(),
            letter_spacing: LetterSpacing::default(),
            bold_is_bright: false,
            ligature_fonts: Vec::new(),
            cursor_style: CursorStyle::default(),
            cursor_blink: true,
            cursor_color: None,
            padding_x: PaddingX::default(),
            padding_y: PaddingY::default(),
            sidebar_width: SidebarWidth::default(),
            sidebar_density: Density::default(),
            ui_scale: UiScale::default(),
            project_tint: false,
            animations: true,
            reduced_motion: false,
        }
    }
}
