//! `cargo xtask import-alacritty-refs`: converts the reference tests of `alacritty_terminal`
//! (Apache-2.0, `tests/ref/<name>/`) into the compact fixtures of `fixtures/vt/alacritty/`
//! (`docs/08` §4, FR-TERM-020).
//!
//! The upstream expected grids are 0.1 to 1 MB of JSON each. They are rewritten as one text line
//! per row (`row |text|W`) followed by one line per run of styled clusters (`  from-to style`,
//! cluster indexes), which is what `crates/ferrum-vt/tests/alacritty_ref.rs` renders from a
//! `Terminal` too. Wide-character spacers are dropped (the cluster carries its width) and leading
//! blank rows (history that was never filled) are omitted.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::out;

/// Palette index of the first bright named color.
const BRIGHT_BASE: u64 = 8;
/// The named colors in palette order.
const NAMED: [&str; 8] = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

/// `value[key]` without the panic: a missing key reads as null.
fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    value.get(key).unwrap_or(&NULL)
}

/// Tests that are not imported, with the reason (kept in sync with `fixtures/vt/THIRD_PARTY.md`).
const EXCLUDED: &[(&str, &str)] = &[
    (
        "selective_erasure",
        "Alacritty does not implement DECSCA, DECSED and DECSEL and keeps the text; Ferrum erases \
         the unprotected cells as xterm does (fixtures decscusr/decsca and erase/decsed, erase/decsel)",
    ),
    (
        "underline",
        "one row feeds SGR 4:3;21 and expects SGR 21 to cancel bold (Alacritty); xterm and Ferrum \
         treat SGR 21 as a double underline (fixtures sgr/underline_double)",
    ),
];

/// One cell as the fixture format needs it.
struct Cluster {
    text: String,
    style: String,
}

/// `Some("i1")`, `Some("#ff00ff")` or `None` for the default color.
fn color_text(value: &Value) -> Result<Option<String>> {
    if let Some(name) = value.get("Named").and_then(Value::as_str) {
        return Ok(match name {
            "Foreground" | "Background" => None,
            _ => {
                let bright = name.strip_prefix("Bright");
                let base = bright.unwrap_or(name);
                let index = NAMED.iter().position(|n| *n == base);
                let Some(index) = index else { bail!("named color {name} is not supported") };
                let offset = if bright.is_some() { BRIGHT_BASE } else { 0 };
                Some(format!("i{}", offset + u64::try_from(index)?))
            }
        });
    }
    if let Some(index) = value.get("Indexed").and_then(Value::as_u64) {
        return Ok(Some(format!("i{index}")));
    }
    if let Some(rgb) = value.get("Spec") {
        let channel = |key: &str| rgb.get(key).and_then(Value::as_u64).context("rgb channel");
        return Ok(Some(format!(
            "#{:02x}{:02x}{:02x}",
            channel("r")?,
            channel("g")?,
            channel("b")?
        )));
    }
    bail!("unknown color {value}")
}

/// The style words of one cell, in the order of `Terminal::dump`.
fn style_text(cell: &Value, flags: &[&str]) -> Result<String> {
    let mut parts = Vec::new();
    if let Some(fg) = color_text(field(cell, "fg"))? {
        parts.push(format!("fg={fg}"));
    }
    if let Some(bg) = color_text(field(cell, "bg"))? {
        parts.push(format!("bg={bg}"));
    }
    for (flag, name) in [
        ("BOLD", "bold"),
        ("DIM", "dim"),
        ("ITALIC", "italic"),
        ("INVERSE", "inverse"),
        ("HIDDEN", "hidden"),
        ("STRIKEOUT", "strike"),
    ] {
        if flags.contains(&flag) {
            parts.push(name.to_owned());
        }
    }
    for (flag, name) in [
        ("UNDERLINE", "single"),
        ("DOUBLE_UNDERLINE", "double"),
        ("UNDERCURL", "curly"),
        ("DOTTED_UNDERLINE", "dotted"),
        ("DASHED_UNDERLINE", "dashed"),
    ] {
        if flags.contains(&flag) {
            parts.push(format!("underline={name}"));
        }
    }
    let extra = &field(cell, "extra");
    if let Some(color) = extra.get("underline_color").filter(|v| !v.is_null())
        && let Some(text) = color_text(color)?
    {
        parts.push(format!("ul={text}"));
    }
    let link = extra.get("hyperlink").and_then(|link| link.get("inner").or(Some(link)));
    if let Some(uri) = link.and_then(|link| link.get("uri")).and_then(Value::as_str) {
        parts.push(format!("link={uri}"));
    }
    Ok(parts.join(" "))
}

/// The clusters of a row and whether it soft-wraps.
fn row_clusters(row: &Value) -> Result<(Vec<Cluster>, bool)> {
    let cells = field(row, "inner").as_array().context("row cells")?;
    let mut clusters = Vec::new();
    let mut wrapped = false;
    for cell in cells {
        let flag_text = field(cell, "flags").as_str().unwrap_or_default();
        let flags: Vec<&str> = flag_text.split(" | ").filter(|f| !f.is_empty()).collect();
        wrapped = flags.contains(&"WRAPLINE");
        if flags.contains(&"WIDE_CHAR_SPACER") || flags.contains(&"LEADING_WIDE_CHAR_SPACER") {
            continue;
        }
        // Alacritty writes the tab character into the first cell a tab crosses; Ferrum leaves
        // the cells blank, so the tab is read as a blank (see THIRD_PARTY.md).
        let mut text = field(cell, "c").as_str().unwrap_or(" ").replace('\t', " ");
        if let Some(extra) = field(field(cell, "extra"), "zerowidth").as_array() {
            text.extend(extra.iter().filter_map(Value::as_str));
        }
        clusters.push(Cluster { text, style: style_text(cell, &flags)? });
    }
    Ok((clusters, wrapped))
}

/// The lines of one row: `row |text|W` and the style runs.
fn row_lines(row: &Value) -> Result<Vec<String>> {
    let (mut clusters, wrapped) = row_clusters(row)?;
    if !wrapped {
        while clusters.last().is_some_and(|c| c.text == " " && c.style.is_empty()) {
            clusters.pop();
        }
    }
    let text: String = clusters.iter().map(|c| c.text.as_str()).collect();
    let mut lines = vec![format!("row |{text}|{}", if wrapped { "W" } else { "" })];
    let mut runs: Vec<(usize, usize, &str)> = Vec::new();
    for (index, cluster) in clusters.iter().enumerate().filter(|(_, c)| !c.style.is_empty()) {
        match runs.last_mut() {
            Some((_, end, style)) if *end + 1 == index && *style == cluster.style => *end = index,
            _ => runs.push((index, index, &cluster.style)),
        }
    }
    lines.extend(runs.into_iter().map(|(from, to, style)| format!("  {from}-{to} {style}")));
    Ok(lines)
}

/// The whole expected file for a grid JSON document.
fn expected(grid: &Value, size: &Value, history: u64) -> Result<String> {
    let storage = &field(grid, "raw");
    let rows = field(storage, "inner").as_array().context("grid rows")?;
    let zero = usize::try_from(field(storage, "zero").as_u64().context("zero")?)?;
    let len = rows.len();
    // Storage is a ring that starts at the bottom screen row and goes up into the history.
    let mut top_down = Vec::with_capacity(len);
    for request in (0..len).rev() {
        top_down.push(row_lines(rows.get((zero + request) % len).context("ring index")?)?);
    }
    let first_used = top_down
        .iter()
        .position(|lines| lines.len() > 1 || lines.first().is_some_and(|l| l != "row ||"))
        .unwrap_or(top_down.len());
    let mut out = format!(
        "# Converted from alacritty_terminal tests/ref (Apache-2.0); see THIRD_PARTY.md.\nsize {}x{} history {history}\n",
        field(size, "columns"),
        field(size, "screen_lines")
    );
    for lines in top_down.iter().skip(first_used) {
        out.push_str(&lines.join("\n"));
        out.push('\n');
    }
    Ok(out)
}

/// Converts every test under `from` into `to`.
pub(crate) fn run(from: &Path, to: &Path) -> Result<bool> {
    fs::create_dir_all(to)?;
    let mut names: Vec<String> = fs::read_dir(from)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("alacritty.recording").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !EXCLUDED.iter().any(|(excluded, _)| excluded == name))
        .collect();
    names.sort();
    for name in &names {
        let dir = from.join(name);
        let read = |file: &str| {
            fs::read_to_string(dir.join(file)).with_context(|| format!("{name}/{file}"))
        };
        let grid: Value = serde_json::from_str(&read("grid.json")?)?;
        let size: Value = serde_json::from_str(&read("size.json")?)?;
        let config: Value = serde_json::from_str(&read("config.json")?)?;
        let history = field(&config, "history_size").as_u64().unwrap_or(0);
        let text = expected(&grid, &size, history).with_context(|| format!("converting {name}"))?;
        fs::write(to.join(format!("{name}.expected")), text)?;
        fs::copy(dir.join("alacritty.recording"), to.join(format!("{name}.recording")))?;
    }
    for (name, reason) in EXCLUDED {
        out::info(&format!("import-alacritty-refs: excluded {name}: {reason}"));
    }
    out::info(&format!(
        "import-alacritty-refs: converted {} tests into {}",
        names.len(),
        to.display()
    ));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cell(c: &str, flags: &str) -> Value {
        json!({"c": c, "fg": {"Named": "Foreground"}, "bg": {"Named": "Background"}, "flags": flags, "extra": null})
    }

    #[test]
    fn colors_flags_and_clusters_are_converted() {
        let red = json!({"c": "r", "fg": {"Named": "BrightRed"}, "bg": {"Spec": {"r": 1, "g": 2, "b": 3}}, "flags": "BOLD | UNDERCURL", "extra": {"zerowidth": ["\u{301}"], "underline_color": {"Indexed": 5}, "hyperlink": {"id": "x", "uri": "https://a.test"}}});
        let row = json!({"inner": [red, cell("漢", "WIDE_CHAR"), cell(" ", "WIDE_CHAR_SPACER"), cell(" ", ""), cell(" ", "")], "occ": 0});
        let lines = row_lines(&row).expect("row");
        assert_eq!(lines[0], "row |r\u{301}漢|");
        assert_eq!(
            lines[1],
            "  0-0 fg=i9 bg=#010203 bold underline=curly ul=i5 link=https://a.test"
        );
    }

    #[test]
    fn wrapped_rows_keep_their_trailing_blanks_and_unknown_colors_fail() {
        let row = json!({"inner": [cell("a", ""), cell(" ", "WRAPLINE")], "occ": 0});
        assert_eq!(row_lines(&row).expect("row")[0], "row |a |W");
        assert!(color_text(&json!({"Named": "DimRed"})).is_err());
    }

    #[test]
    fn the_ring_is_read_from_the_bottom_row_up_and_leading_blank_rows_are_dropped() {
        let rows = json!({"inner": [{"inner": [cell("b", "")]}, {"inner": [cell("a", "")]}, {"inner": [cell(" ", "")]}], "zero": 0, "visible_lines": 2, "len": 3});
        let grid = json!({"raw": rows});
        let text = expected(&grid, &json!({"columns": 1, "screen_lines": 2}), 5).expect("expected");
        assert!(text.ends_with("history 5\nrow |a|\nrow |b|\n"), "{text}");
    }
}
