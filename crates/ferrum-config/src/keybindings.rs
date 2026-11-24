//! The model of `keybindings.toml` (`FR-UI-014`, `FR-DATA-001`, `docs/09` §7).
//!
//! The file holds only the user's changes to the default shortcuts of `docs/09` §7 (the defaults
//! live in the UI's action table, WP-13): each entry binds an action to a chord or unbinds it.
//!
//! ```toml
//! schema_version = 1
//! [[bindings]]
//! action = "new_session"
//! keys = "Ctrl+Shift+T"
//! [[bindings]]
//! action = "zoom_pane"          # no `keys`: the action is unbound
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::schema::Schema;

/// File name of the keybindings file inside the per-user config directory (`docs/02` §8).
pub const KEYBINDINGS_FILE: &str = "keybindings.toml";

/// Versions of `keybindings.toml`. Version 1 is the first; there is nothing to migrate yet.
pub const KEYBINDINGS_SCHEMA: Schema = Schema { name: "keybindings", current: 1, migrations: &[] };

/// Longest key name or action name accepted (anti-hostile cap: real names such as `PageDown` or
/// `prompt_previous` are under 20 characters).
const MAX_NAME_CHARS: usize = 64;

/// Modifier bit of Ctrl.
const CTRL: u8 = 1;
/// Modifier bit of Alt (Option on macOS).
const ALT: u8 = 1 << 1;
/// Modifier bit of Shift.
const SHIFT: u8 = 1 << 2;
/// Modifier bit of Super (Cmd on macOS, Windows key).
const SUPER: u8 = 1 << 3;

/// Modifier names in the order a chord is written, with accepted aliases.
const MODIFIERS: [(u8, &str, &[&str]); 4] = [
    (CTRL, "Ctrl", &["ctrl", "control"]),
    (ALT, "Alt", &["alt", "opt", "option"]),
    (SHIFT, "Shift", &["shift"]),
    (SUPER, "Super", &["super", "cmd", "command", "win", "meta"]),
];

/// A key with modifiers, written like `Ctrl+Shift+T` (case-insensitive modifiers, any order).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Chord {
    modifiers: u8,
    key: String,
}

impl Chord {
    /// Parses `Mod+Mod+Key`; the key `+` is written `Ctrl++`.
    ///
    /// ```
    /// use ferrum_config::Chord;
    /// let chord = Chord::parse("shift+ctrl+t").unwrap();
    /// assert_eq!(chord.to_string(), "Ctrl+Shift+t");
    /// assert_eq!(Chord::parse("Ctrl++").unwrap().key(), "+");
    /// assert!(Chord::parse("Ctrl+").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let (mods, key) = match text.strip_suffix("++") {
            Some(rest) => (Some(rest), "+"),
            None => text.rsplit_once('+').map_or((None, text), |(m, k)| (Some(m), k)),
        };
        let valid_key = !key.is_empty()
            && key.chars().count() <= MAX_NAME_CHARS
            && !key.chars().any(|c| c.is_control() || c.is_whitespace());
        if !valid_key {
            return None;
        }
        let mut modifiers = 0;
        for name in mods.into_iter().flat_map(|m| m.split('+')) {
            let lower = name.to_ascii_lowercase();
            let (bit, _, _) =
                MODIFIERS.iter().find(|(_, _, aliases)| aliases.contains(&lower.as_str()))?;
            if modifiers & bit != 0 {
                return None;
            }
            modifiers |= bit;
        }
        Some(Self { modifiers, key: key.to_owned() })
    }

    /// The key without modifiers.
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (bit, name, _) in MODIFIERS {
            if self.modifiers & bit != 0 {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&self.key)
    }
}

impl TryFrom<String> for Chord {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| format!("{text:?} is not a key chord like Ctrl+Shift+T"))
    }
}

impl From<Chord> for String {
    fn from(chord: Chord) -> Self {
        chord.to_string()
    }
}

/// Name of a UI action (`snake_case` letters, digits and `_`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ActionName(String);

impl ActionName {
    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ActionName {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        let valid = !name.is_empty()
            && name.len() <= MAX_NAME_CHARS
            && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if valid { Ok(Self(name)) } else { Err(format!("{name:?} is not an action name")) }
    }
}

impl From<ActionName> for String {
    fn from(name: ActionName) -> Self {
        name.0
    }
}

/// One user change to the default shortcuts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Binding {
    /// The action.
    pub action: ActionName,
    /// Its new chord; `None` unbinds the action.
    #[serde(default)]
    pub keys: Option<Chord>,
}

/// All user changes to the default shortcuts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Keybindings {
    /// Changes, applied in order (a later entry for the same action wins).
    pub bindings: Vec<Binding>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_parse_aliases_and_print_canonically() {
        let chord = Chord::parse("cmd+opt+Down").unwrap();
        assert_eq!(chord.to_string(), "Alt+Super+Down");
        assert_eq!(Chord::parse("F2").unwrap().to_string(), "F2");
        assert_eq!(Chord::parse("Ctrl+=").unwrap().key(), "=");
    }

    #[test]
    fn malformed_chords_are_rejected() {
        let long = format!("Ctrl+{}", "k".repeat(MAX_NAME_CHARS + 1));
        for bad in ["", "Ctrl+", "Hyper+A", "Ctrl+Ctrl+A", "Ctrl+Space bar", "Ctrl+\t", "+A", &long]
        {
            assert!(Chord::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn action_names_are_snake_case() {
        assert!(ActionName::try_from("split_right".to_owned()).is_ok());
        for bad in ["", "Split", "split-right", "a b"] {
            assert!(ActionName::try_from(bad.to_owned()).is_err(), "{bad}");
        }
    }
}
