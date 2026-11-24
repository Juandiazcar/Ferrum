//! The model of a theme file, `themes/<name>.toml` (`FR-UI-008`, `docs/09` §9).
//!
//! ```toml
//! schema_version = 1
//! [meta]
//! name = "Ferrum Dark"
//! variant = "dark"
//! [colors]
//! foreground = "#d7dae0"
//! background = "#101214"
//! # cursor, cursor_text, selection_bg, selection_fg, ansi = [16 colors], bright_foreground
//! [ui]
//! # sidebar_bg, sidebar_fg, accent, border, status_bg, status_fg, danger, warning, success
//! [fonts]          # optional
//! family = "JetBrains Mono"
//! ```
//!
//! v1.0 has dark themes only (`docs/00` §5), so `variant` accepts only `dark`. The built-in
//! themes and the importers arrive with WP-13.

use serde::{Deserialize, Serialize};

use crate::schema::Schema;
use crate::settings::FontSize;
use crate::value::Rgb;

/// Directory of user themes inside the per-user config directory (`docs/02` §8).
pub const THEMES_DIR: &str = "themes";

/// Versions of a theme file. Version 1 is the first; there is nothing to migrate yet.
pub const THEME_SCHEMA: Schema = Schema { name: "theme", current: 1, migrations: &[] };

/// Colors in the ANSI palette: 8 normal and 8 bright (`FR-TERM-002`).
pub const ANSI_COLORS: usize = 16;

/// Theme variant. Only dark themes exist in v1.0 (`docs/00` §5, ADR 0018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Variant {
    /// Dark background.
    #[default]
    Dark,
}

/// `[meta]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThemeMeta {
    /// Display name.
    pub name: String,
    /// Author.
    #[serde(default)]
    pub author: Option<String>,
    /// Variant.
    #[serde(default)]
    pub variant: Variant,
}

/// `[colors]`: terminal colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TerminalColors {
    /// Default text color.
    pub foreground: Rgb,
    /// Default background color.
    pub background: Rgb,
    /// Cursor color.
    pub cursor: Rgb,
    /// Text under a block cursor.
    pub cursor_text: Rgb,
    /// Selection background.
    pub selection_bg: Rgb,
    /// Selection text.
    pub selection_fg: Rgb,
    /// ANSI palette, colors 0–15.
    pub ansi: [Rgb; ANSI_COLORS],
    /// Foreground for bold text when "bold is bright" is on; `None` uses `ansi[15]`.
    #[serde(default)]
    pub bright_foreground: Option<Rgb>,
}

/// `[ui]`: chrome colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UiColors {
    /// Sidebar background.
    pub sidebar_bg: Rgb,
    /// Sidebar text.
    pub sidebar_fg: Rgb,
    /// Accent (focus, selection in lists).
    pub accent: Rgb,
    /// Borders and dividers.
    pub border: Rgb,
    /// Status bar background.
    pub status_bg: Rgb,
    /// Status bar text.
    pub status_fg: Rgb,
    /// Destructive actions.
    pub danger: Rgb,
    /// Warnings.
    pub warning: Rgb,
    /// Success states.
    pub success: Rgb,
}

/// `[fonts]`: optional font overrides of a theme.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ThemeFonts {
    /// Font family.
    pub family: Option<String>,
    /// Font size.
    pub size: Option<FontSize>,
}

/// A theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Theme {
    /// Name and variant.
    pub meta: ThemeMeta,
    /// Terminal colors.
    pub colors: TerminalColors,
    /// Chrome colors.
    pub ui: UiColors,
    /// Font overrides.
    #[serde(default)]
    pub fonts: Option<ThemeFonts>,
}
