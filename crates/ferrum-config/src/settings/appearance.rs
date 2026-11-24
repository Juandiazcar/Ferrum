//! Settings → Appearance (`docs/09` §8, `docs/09` §9).

use serde::{Deserialize, Serialize};

use crate::value::{Rgb, bounded};

/// Name of the default theme (`docs/09` §9).
pub const DEFAULT_THEME: &str = "Ferrum Dark";

/// Default font family: the embedded variable font (`docs/09` §9, `docs/07` §5).
pub const DEFAULT_FONT_FAMILY: &str = "JetBrains Mono";

bounded!(
    /// Terminal font size in points.
    FontSize(f32),
    min = 6.0,
    max = 72.0,
    default = 13.0
);
bounded!(
    /// Line height as a multiple of the font's natural height.
    LineHeight(f32),
    min = 0.8,
    max = 2.0,
    default = 1.0
);
bounded!(
    /// Extra space between cells in logical pixels.
    LetterSpacing(f32),
    min = -2.0,
    max = 10.0,
    default = 0.0
);
bounded!(
    /// Horizontal window padding in logical pixels (`docs/09` §5: default 8 px).
    PaddingX(u16),
    min = 0,
    max = 64,
    default = 8
);
bounded!(
    /// Vertical window padding in logical pixels (`docs/09` §5: default 6 px).
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
    /// UI scale in percent (`docs/09` §11: 75–200 %).
    UiScale(u16),
    min = 75,
    max = 200,
    default = 100
);

/// Cursor shape (`FR-TERM-017`).
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

/// Sidebar row density.
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
    /// Draw bold text in the bright palette colors.
    pub bold_is_bright: bool,
    /// Font ligatures (`FR-TERM-019`: off by default).
    pub ligatures: bool,
    /// Cursor shape.
    pub cursor_style: CursorStyle,
    /// Blink the cursor while the window is focused (`FR-TERM-017`, `NFR-PERF-06`).
    pub cursor_blink: bool,
    /// Cursor color; `None` uses the theme's cursor color.
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
    /// UI transitions (`docs/09` §10: optional).
    pub animations: bool,
    /// Reduced motion (`docs/09` §11).
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
            ligatures: false,
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
