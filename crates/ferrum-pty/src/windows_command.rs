//! Pure parts of the Windows spawn (ADR 0026: "command-line quoting, PATH search and environment
//! block"), written over UTF-16 units so they are tested on every OS.
//!
//! Quoting follows the rules `CommandLineToArgvW` and the Microsoft C runtime use to split a
//! command line, the same rules as `std::process::Command`. The environment block is the
//! inherited environment with the spawn's changes applied, matched without regard to case, sorted
//! as `CreateProcessW` requires, and terminated by an empty string.

use std::cmp::Ordering;

use crate::PtyError;

/// Longest command line `CreateProcessW` accepts, terminating NUL included.
const MAX_COMMAND_LINE_UNITS: usize = 32_767;

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const EQUALS: u16 = b'=' as u16;

/// One variable of an environment block, as UTF-16 units without terminators.
pub(crate) type Variable = (Vec<u16>, Vec<u16>);

/// Builds the NUL-terminated command line: the program, then each argument, quoted as needed.
///
/// # Errors
/// [`PtyError::InvalidSpec`] when the result is longer than `CreateProcessW` accepts.
pub(crate) fn command_line(program: &[u16], args: &[Vec<u16>]) -> Result<Vec<u16>, PtyError> {
    let mut line = Vec::new();
    append_quoted(&mut line, program);
    for arg in args {
        line.push(SPACE);
        append_quoted(&mut line, arg);
    }
    line.push(0);
    if line.len() > MAX_COMMAND_LINE_UNITS {
        return Err(PtyError::InvalidSpec("command line too long"));
    }
    Ok(line)
}

fn append_quoted(line: &mut Vec<u16>, arg: &[u16]) {
    let quote = arg.is_empty() || arg.iter().any(|&unit| unit == SPACE || unit == TAB);
    if quote {
        line.push(QUOTE);
    }
    let mut backslashes = 0;
    for &unit in arg {
        if unit == BACKSLASH {
            backslashes += 1;
        } else {
            if unit == QUOTE {
                // Backslashes before a quote are doubled, plus one to escape the quote.
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes + 1));
            }
            backslashes = 0;
        }
        line.push(unit);
    }
    if quote {
        // Backslashes before the closing quote are doubled so they stay literal.
        line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
        line.push(QUOTE);
    }
}

/// Characters an unquoted batch-file argument may contain besides ASCII letters and digits
/// (the set `std::process::Command` uses).
const BATCH_UNQUOTED: &str = r"#$*+-./:?@\_";

/// What `cmd.exe` expands for `%`: `%%cd:~,%` is an empty substring of `%cd%`, which stops
/// `%NAME%` in an argument from being expanded (the technique `std::process::Command` uses).
const BATCH_PERCENT: &str = "%%cd:~,";

/// Builds the command line that runs batch file `script` with `args` through `cmd.exe`, escaped
/// so that `cmd.exe` passes every argument through literally, with the same rules as
/// `std::process::Command` (`cmd.exe /e:ON /v:OFF /d /c ""script" args"`).
///
/// # Errors
/// [`PtyError::InvalidSpec`] when the script path contains `"`, `^` or `%` or ends with `\`, when
/// an argument contains a line break (`cmd.exe` would cut the line there), or when the line is
/// too long.
pub(crate) fn batch_command_line(script: &[u16], args: &[Vec<u16>]) -> Result<Vec<u16>, PtyError> {
    let forbidden = |unit: &u16| [u16::from(b'"'), u16::from(b'^'), u16::from(b'%')].contains(unit);
    if script.iter().any(forbidden) || script.last() == Some(&BACKSLASH) {
        return Err(PtyError::InvalidSpec("batch file path cannot be passed to cmd.exe"));
    }
    let mut line: Vec<u16> = "cmd.exe /e:ON /v:OFF /d /c \"\"".encode_utf16().collect();
    line.extend_from_slice(script);
    line.push(QUOTE);
    for arg in args {
        if arg.iter().any(|&unit| unit == u16::from(b'\r') || unit == u16::from(b'\n')) {
            return Err(PtyError::InvalidSpec("line break in a batch file argument"));
        }
        line.push(SPACE);
        append_batch_arg(&mut line, arg);
    }
    line.push(QUOTE);
    line.push(0);
    if line.len() > MAX_COMMAND_LINE_UNITS {
        return Err(PtyError::InvalidSpec("command line too long"));
    }
    Ok(line)
}

fn append_batch_arg(line: &mut Vec<u16>, arg: &[u16]) {
    let needs_quotes = |c: char| {
        (c.is_ascii() && !(c.is_ascii_alphanumeric() || BATCH_UNQUOTED.contains(c)))
            || c.is_control()
    };
    let quote = arg.is_empty()
        || arg.last() == Some(&BACKSLASH)
        || char::decode_utf16(arg.iter().copied()).any(|c| c.is_ok_and(needs_quotes));
    if quote {
        line.push(QUOTE);
    }
    let mut backslashes = 0;
    for &unit in arg {
        if unit == BACKSLASH {
            backslashes += 1;
        } else {
            if unit == QUOTE {
                // Backslashes before a quote are doubled, and the quote is escaped by doubling.
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
                line.push(QUOTE);
            } else if unit == u16::from(b'%') {
                line.extend(BATCH_PERCENT.encode_utf16());
            }
            backslashes = 0;
        }
        line.push(unit);
    }
    if quote {
        line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
        line.push(QUOTE);
    }
}

/// Applies `changes` (`None` removes) over `inherited`, matching names without regard to case.
pub(crate) fn apply_changes(
    mut variables: Vec<Variable>,
    changes: impl IntoIterator<Item = (Vec<u16>, Option<Vec<u16>>)>,
) -> Vec<Variable> {
    for (name, value) in changes {
        variables.retain(|(existing, _)| compare_names(existing, &name) != Ordering::Equal);
        if let Some(value) = value {
            variables.push((name, value));
        }
    }
    variables
}

/// The value of `name` (matched without regard to case).
pub(crate) fn lookup<'a>(variables: &'a [Variable], name: &str) -> Option<&'a [u16]> {
    let name: Vec<u16> = name.encode_utf16().collect();
    variables
        .iter()
        .find(|(existing, _)| compare_names(existing, &name) == Ordering::Equal)
        .map(|(_, value)| value.as_slice())
}

/// The environment block: `name=value\0` for each variable sorted by name, then a final `\0`.
pub(crate) fn environment_block(mut variables: Vec<Variable>) -> Vec<u16> {
    variables.sort_by(|(left, _), (right, _)| compare_names(left, right));
    let mut block = Vec::new();
    for (name, value) in variables {
        block.extend(name);
        block.push(EQUALS);
        block.extend(value);
        block.push(0);
    }
    if block.is_empty() {
        // An empty block is still two NULs: an empty string, then the terminator.
        block.push(0);
    }
    block.push(0);
    block
}

/// Case-insensitive order of variable names (Windows treats `Path` and `PATH` as one variable).
fn compare_names(left: &[u16], right: &[u16]) -> Ordering {
    let upper = |units: &[u16]| {
        char::decode_utf16(units.iter().copied())
            .flat_map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER).to_uppercase())
            .collect::<Vec<char>>()
    };
    upper(left).cmp(&upper(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn line(program: &str, args: &[&str]) -> String {
        let args: Vec<Vec<u16>> = args.iter().map(|arg| wide(arg)).collect();
        let line = command_line(&wide(program), &args).unwrap();
        String::from_utf16(line.strip_suffix(&[0]).unwrap()).unwrap()
    }

    #[test]
    fn quotes_like_the_c_runtime_splits() {
        assert_eq!(
            line(r"C:\bin\a.exe", &["x", "", "two words"]),
            r#"C:\bin\a.exe x "" "two words""#
        );
        assert_eq!(line("a", &[r#"say "hi""#]), r#"a "say \"hi\"""#);
        assert_eq!(line("a", &[r"trailing\"]), r"a trailing\");
        assert_eq!(line("a", &[r"dir with space\"]), r#"a "dir with space\\""#);
        assert_eq!(line("a", &[r#"back\"quote"#]), r#"a back\\\"quote"#);
        assert_eq!(line(r"C:\Program Files\a.exe", &[]), r#""C:\Program Files\a.exe""#);
    }

    fn batch(script: &str, args: &[&str]) -> Result<String, PtyError> {
        let args: Vec<Vec<u16>> = args.iter().map(|arg| wide(arg)).collect();
        let line = batch_command_line(&wide(script), &args)?;
        Ok(String::from_utf16(line.strip_suffix(&[0]).unwrap()).unwrap())
    }

    #[test]
    fn batch_files_run_through_cmd_with_literal_arguments() {
        let line =
            batch(r"C:\b\run.cmd", &["plain", "two words", "100%", r#"say "hi""#, ""]).unwrap();
        assert_eq!(
            line,
            r#"cmd.exe /e:ON /v:OFF /d /c ""C:\b\run.cmd" plain "two words" "100%%cd:~,%" "say ""hi""" """"#
        );
        assert!(batch(r"C:\b\run.cmd", &["a\nb"]).is_err());
        assert!(batch(r"C:\b\100%.cmd", &[]).is_err());
    }

    #[test]
    fn rejects_a_command_line_longer_than_windows_accepts() {
        let long = vec![u16::from(b'a'); MAX_COMMAND_LINE_UNITS];
        assert!(command_line(&wide("a"), &[long]).is_err());
    }

    #[test]
    fn applies_changes_without_regard_to_case_and_sorts() {
        let inherited = vec![(wide("Path"), wide("C:\\bin")), (wide("TERM"), wide("xterm"))];
        let changes = [(wide("PATH"), None), (wide("term"), None), (wide("B"), Some(wide("2")))];
        let mut variables = apply_changes(inherited, changes);
        variables.push((wide("a"), wide("1")));
        assert_eq!(lookup(&variables, "b"), Some(wide("2").as_slice()));
        assert_eq!(lookup(&variables, "path"), None);
        let block = String::from_utf16(&environment_block(variables)).unwrap();
        assert_eq!(block, "a=1\0B=2\0\0");
    }

    #[test]
    fn an_empty_environment_is_two_nuls() {
        assert_eq!(environment_block(Vec::new()), vec![0, 0]);
    }
}
