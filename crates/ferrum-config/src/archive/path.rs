//! Which entry names an archive may contain (`docs/10`: never trust an imported path).
//!
//! A name must be safe to create on Linux, macOS and Windows below the staging directory: no
//! empty, `.` or `..` components, no hidden names, no separators or drive letters, no characters
//! Windows rejects, no Windows device names, bounded length and depth.

use unicode_normalization::UnicodeNormalization;

use super::Section;

/// Longest component in bytes: the name limit of ext4, APFS and NTFS.
const MAX_COMPONENT_BYTES: usize = 255;

/// Most components below the section directory. Anti-hostile cap: Ferrum's own layout is at most
/// two deep (`config/themes/x.toml`); 8 leaves room for user-made folders.
pub(super) const MAX_DEPTH: usize = 8;

/// Characters that are separators, drive or stream markers, or invalid on Windows.
const FORBIDDEN_CHARS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Windows device names, reserved with any extension and in any case (Win32 path rules: `COM0`
/// to `COM9`, `LPT0` to `LPT9`, the superscript digits ¹ ² ³, and the console names).
const RESERVED_STEMS: [&str; 34] = [
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM0", "COM1", "COM2", "COM3", "COM4",
    "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3",
    "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³", "CLOCK$", "$MFT",
];

/// Whether `name` is a hidden name (temporary files of atomic writes are hidden).
pub(super) fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Whether `name` is allowed as one path component.
pub(super) fn is_portable_component(name: &str) -> bool {
    // Windows ignores trailing spaces and dots of the stem: `CON .toml` is the device CON.
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches([' ', '.']);
    !name.is_empty()
        && name.len() <= MAX_COMPONENT_BYTES
        && !is_hidden(name)
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| c.is_control() || FORBIDDEN_CHARS.contains(&c))
        && !RESERVED_STEMS.iter().any(|reserved| reserved.eq_ignore_ascii_case(stem))
}

/// The key under which two paths are the same file on a case-insensitive, normalization-
/// insensitive file system (APFS, NTFS): Unicode NFC, then lowercase.
pub(super) fn collision_key(path: &str) -> String {
    path.nfc().collect::<String>().to_lowercase()
}

/// Splits an entry path `section/a/b` into its section and checked components.
pub(super) fn parse_entry(path: &str) -> Option<(Section, Vec<&str>)> {
    let mut parts = path.split('/');
    let section = Section::from_dir_name(parts.next()?)?;
    let rest: Vec<&str> = parts.collect();
    let valid = !rest.is_empty()
        && rest.len() <= MAX_DEPTH
        && rest.iter().all(|part| is_portable_component(part));
    valid.then_some((section, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_names_are_accepted() {
        for name in
            ["settings.toml", "Tokyo Night.toml", "ñandú.toml", "a", "x.y.z", "CONSOLE.toml"]
        {
            assert!(is_portable_component(name), "{name}");
        }
    }

    #[test]
    fn unsafe_names_are_rejected() {
        let long = "a".repeat(MAX_COMPONENT_BYTES + 1);
        for name in [
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a\\b",
            "C:",
            "c:x",
            "a*",
            "a?",
            "a\"",
            "a<",
            "a>",
            "a|",
            "a\u{0}",
            "a\n",
            "trailing.",
            "trailing ",
            "CON",
            "con.toml",
            "Lpt1.txt",
            "CON .toml",
            "nul. .txt",
            "COM0",
            "com¹.toml",
            "LPT³",
            "CONIN$",
            "conout$.log",
            &long,
        ] {
            assert!(!is_portable_component(name), "{name:?}");
        }
    }

    #[test]
    fn collision_keys_fold_case_and_normalization() {
        assert_eq!(collision_key("Themes/Nord.toml"), collision_key("themes/NORD.toml"));
        assert_eq!(collision_key("\u{c5}.toml"), collision_key("A\u{30a}.toml"));
        assert_ne!(collision_key("a.toml"), collision_key("b.toml"));
    }

    #[test]
    fn entry_paths_need_a_known_section_and_safe_components() {
        let (section, rest) = parse_entry("data/projects/a.toml").unwrap();
        assert_eq!((section, rest), (Section::Data, vec!["projects", "a.toml"]));
        let deep = format!("config/{}", ["d"; MAX_DEPTH + 1].join("/"));
        for bad in [
            "config",
            "config/",
            "other/a",
            "/config/a",
            "config/../a",
            "config//a",
            "config/./a",
            "./config/a",
            "config/a/",
            &deep,
        ] {
            assert!(parse_entry(bad).is_none(), "{bad}");
        }
    }
}
