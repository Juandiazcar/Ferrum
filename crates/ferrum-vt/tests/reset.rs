//! RIS (`ESC c`) resets exactly what Alacritty's `Term::reset_state` resets (maintainer decisions
//! Q-23 and Q-25: the Alacritty reference behavior, literally). Like Alacritty it sends no event
//! for the title and keeps the palette (`docs/user/compat.md`).

use ferrum_scrollback::{Scrollback, ScrollbackConfig};
use ferrum_vt::{Event, EventSink, Size, Terminal};

#[derive(Default)]
struct Log(Vec<String>);

impl EventSink for Log {
    fn event(&mut self, event: Event<'_>) {
        self.0.push(format!("{event:?}"));
    }
}

fn terminal() -> Terminal<Log> {
    Terminal::new(Size::new(20, 4), Scrollback::new(ScrollbackConfig::default()), Log::default())
}

#[test]
fn ris_resets_the_title_without_an_event() {
    let mut term = terminal();
    term.parse(b"\x1b]2;build\x07");
    assert_eq!(term.title(), "build");
    let events = term.sink().0.len();
    term.parse(b"\x1bc");
    assert_eq!(term.title(), "");
    assert_eq!(term.sink().0.len(), events, "{:?}", term.sink().0);
}

#[test]
fn ris_keeps_the_palette_and_the_working_directory() {
    let mut term = terminal();
    term.parse(b"\x1b]4;1;rgb:12/34/56\x07\x1b]7;file://host/tmp\x07");
    let palette = format!("{:?}", term.palette());
    term.parse(b"\x1bc");
    assert_eq!(format!("{:?}", term.palette()), palette);
    assert_eq!(term.cwd(), "file://host/tmp");
}
