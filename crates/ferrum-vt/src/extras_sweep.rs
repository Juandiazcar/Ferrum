//! Releasing hyperlink and underline-color ids that no row uses any more (`extras.rs`).
//!
//! The ids travel inside cell and history style words, so an id may be handed out again only
//! when nothing carries it. A sweep marks every holder (both screens' cells, every pen and saved
//! cursor, and the per-block summaries of the retained history, `history_extras.rs`) and
//! releases the rest. It never reads the history back, never allocates, and runs only when the
//! table is full and the cooldown of the previous sweep (its work over `WORK_PER_REFUSAL`, minus
//! what it freed) has been spent.

use crate::color::Color;
use crate::core::Core;
use crate::cursor::Pen;
use crate::events::EventSink;
use crate::extras::{ExtraId, LinkId};
use crate::screen::Screen;
use crate::slot_set::SlotSet;

/// The ids found in use so far.
struct Marks {
    extras: SlotSet,
    links: SlotSet,
}

impl Marks {
    fn pen(&mut self, pen: &Pen) {
        self.extras.insert(usize::from(pen.extra.bits()));
        self.links.insert(usize::from(pen.link.bits()));
    }

    /// Marks the ids of a screen; returns the cells scanned.
    fn screen(&mut self, screen: &Screen) -> usize {
        self.pen(&screen.cursor.pen);
        if let Some(saved) = &screen.saved {
            self.pen(&saved.cursor.pen);
        }
        let grid = &screen.grid;
        let mut cells = 0;
        for row in (0..grid.lines()).filter_map(|y| grid.row(y)) {
            for cell in row.cells() {
                self.extras.insert(usize::from(cell.extra().bits()));
            }
            cells += row.len();
        }
        cells
    }
}

impl<S: EventSink> Core<S> {
    /// The id of the combination `link` + `underline`, sweeping unused ids when the table is
    /// full. `ExtraId::NONE` (and a counted refusal) when it is still full.
    pub(crate) fn intern_extra(&mut self, link: LinkId, underline: Color) -> ExtraId {
        if let Some(id) = self.extras.try_intern(link, underline) {
            return id;
        }
        if self.extras.sweep_due() {
            self.sweep_extras(link);
            if let Some(id) = self.extras.try_intern(link, underline) {
                return id;
            }
        }
        self.extras.note_refused();
        ExtraId::NONE
    }

    /// The link for `uri`, sweeping unused ids when the table is full. `LinkId::NONE` (and a
    /// counted refusal) when it is still full.
    pub(crate) fn intern_link_swept(&mut self, id: &str, uri: &str) -> LinkId {
        if let Some(link) = self.extras.try_intern_link(id, uri, &mut self.scratch.short) {
            return link;
        }
        if self.extras.sweep_due() {
            self.sweep_extras(LinkId::NONE);
            if let Some(link) = self.extras.try_intern_link(id, uri, &mut self.scratch.short) {
                return link;
            }
        }
        self.extras.note_refused();
        LinkId::NONE
    }

    /// Releases the ids nothing uses. `pending` is a link that is about to be stored in a pen.
    /// Returns how many entries were released.
    pub(crate) fn sweep_extras(&mut self, pending: LinkId) -> usize {
        let mut marks = Marks { extras: SlotSet::new(), links: SlotSet::new() };
        marks.links.insert(usize::from(pending.bits()));
        let mut work = marks.screen(&self.primary);
        if let Some(alt) = &self.alt {
            work += marks.screen(alt);
        }
        if let Some(saved) = &self.alt_saved {
            marks.pen(&saved.cursor.pen);
        }
        work += self.extras.history.mark(&mut marks.extras);
        let freed = self.extras.release_unmarked(&marks.extras, marks.links);
        self.extras.sweep_done(work, freed);
        freed
    }
}

#[cfg(test)]
mod tests {
    use ferrum_scrollback::{Scrollback, ScrollbackConfig};

    use crate::limits::{MAX_EXTRAS, WORK_PER_REFUSAL};
    use crate::{NullSink, Size, Terminal};

    /// Requests of the hostile stream.
    const COLORS: usize = 2_000;
    /// Work allowed per request above `WORK_PER_REFUSAL`: the last sweep is not yet amortized.
    const SLACK: u64 = 4;

    #[test]
    fn a_full_table_bounds_the_sweep_work_of_each_underline_color() {
        let config = ScrollbackConfig::new(10_000, 50_000).unwrap_or_default();
        let mut term = Terminal::new(Size::new(80, 24), Scrollback::new(config), NullSink);
        for n in 0..MAX_EXTRAS - 1 {
            term.parse(format!("\x1b]8;;https://x.test/{n}\x07x\x1b]8;;\x07\r\n").as_bytes());
        }
        let before = term.core.extras.sweep_work();
        for n in 0..COLORS {
            let [_, r, g, b] = u32::try_from(n).unwrap_or(0).to_be_bytes();
            term.parse(format!("\x1b[1;1H\x1b[58;2;{r};{g};{b}mx").as_bytes());
        }
        let work = term.core.extras.sweep_work() - before;
        assert!(work > 0, "the stream sweeps");
        let per_color = work / u64::try_from(COLORS).unwrap_or(1);
        assert!(per_color <= WORK_PER_REFUSAL as u64 + SLACK, "{per_color} units per color");
    }
}
