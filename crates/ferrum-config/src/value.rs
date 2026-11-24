//! Validated value types shared by the settings and project models.
//!
//! Each type checks its range when deserialized, so an out-of-range value in a file is reported
//! as [`ConfigError::Invalid`](crate::ConfigError::Invalid) instead of reaching the program.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A 24-bit color written as `"#rrggbb"` in files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// Hex digits of a `#rrggbb` color after the `#`.
const RGB_HEX_DIGITS: usize = 6;

impl Rgb {
    /// Parses `#rrggbb` (case-insensitive).
    ///
    /// ```
    /// use ferrum_config::Rgb;
    /// assert_eq!(Rgb::parse("#E8590C"), Some(Rgb { r: 0xE8, g: 0x59, b: 0x0C }));
    /// assert_eq!(Rgb::parse("E8590C"), None);
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != RGB_HEX_DIGITS || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        Some(Self { r: channel(0)?, g: channel(2)?, b: channel(4)? })
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| format!("{text:?} is not a #rrggbb color"))
    }
}

impl From<Rgb> for String {
    fn from(color: Rgb) -> Self {
        color.to_string()
    }
}

/// Declares a newtype over a number that only holds values in `MIN..=MAX`, with a default.
macro_rules! bounded {
    ($(#[$doc:meta])* $name:ident($inner:ty), min = $min:expr, max = $max:expr, default = $default:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            /// Smallest allowed value.
            pub const MIN: $inner = $min;
            /// Largest allowed value.
            pub const MAX: $inner = $max;
            /// Value used when the setting is absent.
            pub const DEFAULT: $inner = $default;

            /// Returns the value if it is in `MIN..=MAX`.
            pub fn new(value: $inner) -> Option<Self> {
                (Self::MIN..=Self::MAX).contains(&value).then_some(Self(value))
            }

            /// The value.
            pub fn get(self) -> $inner {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self(Self::DEFAULT)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                ::serde::Serialize::serialize(&self.0, serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <$inner as ::serde::Deserialize>::deserialize(deserializer)?;
                Self::new(value).ok_or_else(|| {
                    <D::Error as ::serde::de::Error>::custom(format!(
                        "{value} is outside {}..={}",
                        Self::MIN,
                        Self::MAX
                    ))
                })
            }
        }
    };
}

pub(crate) use bounded;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::UiScale as Percent;

    #[test]
    fn rgb_round_trips_and_rejects_malformed_text() {
        let color = Rgb::parse("#e8590C").unwrap();
        assert_eq!(color.to_string(), "#e8590c");
        for bad in ["", "#", "#12345", "#1234567", "#12345g", "123456", "#+12345"] {
            assert_eq!(Rgb::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn bounded_values_outside_the_range_are_rejected() {
        assert_eq!(Percent::default().get(), 100);
        assert_eq!(Percent::new(75).map(Percent::get), Some(75));
        assert!(Percent::new(74).is_none() && Percent::new(201).is_none());
        let err = toml::from_str::<std::collections::BTreeMap<String, Percent>>("p = 300");
        assert!(err.unwrap_err().to_string().contains("75..=200"));
    }
}
