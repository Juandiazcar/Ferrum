//! The working directory a shell reports with `OSC 7` (`file://host/path`, percent-encoded), as
//! a path to remember for the session (FR-SESS-008: shells are respawned in their last cwd).

use std::path::PathBuf;

const SCHEME: &str = "file://";
/// Radix of a percent escape.
const HEX: u32 = 16;

/// The local path of an `OSC 7` URI; `None` when it is not a `file` URI with an absolute path or
/// is badly escaped. The host part is ignored (the shell runs on this machine or reports its
/// remote host; either way only the path is kept).
pub(crate) fn from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix(SCHEME)?;
    let path = rest.get(rest.find('/')?..)?;
    let decoded = percent_decode(path)?;
    let text = String::from_utf8(decoded).ok()?;
    // Windows shells send `/C:/Users/...`: the drive comes after the slash.
    let local = match text.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() && cfg!(windows) => text.get(1..)?,
        _ => text.as_str(),
    };
    let path = PathBuf::from(local);
    path.is_absolute().then_some(path)
}

fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            out.push(byte);
            continue;
        }
        let high = char::from(bytes.next()?).to_digit(HEX)?;
        let low = char::from(bytes.next()?).to_digit(HEX)?;
        out.push(u8::try_from(high * HEX + low).ok()?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc7_uris_become_paths() {
        #[cfg(unix)]
        assert_eq!(from_uri("file://host/home/a%20b"), Some(PathBuf::from("/home/a b")));
        #[cfg(windows)]
        assert_eq!(from_uri("file://host/C:/Users/a%20b"), Some(PathBuf::from("C:/Users/a b")));
        assert_eq!(from_uri("http://host/x"), None);
        assert_eq!(from_uri("file://host/bad%2"), None);
        assert_eq!(from_uri("file://host"), None);
    }
}
