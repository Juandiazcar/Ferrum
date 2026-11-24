//! Project layouts: a tree of splits whose leaves are sessions (`FR-PROJ-005`, `docs/09` §4).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::project::SessionKey;

/// Deepest nesting of splits accepted. Anti-hostile cap: 32 nested splits are far more than fit
/// on a screen at the minimum pane size of 10×3 cells (`docs/09` §4), and the balanced trees of
/// [`Layout::equal`] need only 10 levels for 1 024 sessions.
pub const MAX_LAYOUT_DEPTH: usize = 32;

/// Split ratios are per mille of the parent's size given to the first child. A file format choice:
/// integers round-trip exactly through TOML, and 0.1 % of a 4K-wide window is under 4 pixels, finer
/// than a cell.
pub const RATIO_SCALE: u16 = 1_000;

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// Children side by side (split right).
    Horizontal,
    /// Children on top of each other (split down).
    Vertical,
}

/// A layout tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// A pane showing one session.
    Pane {
        /// The session shown.
        session: SessionKey,
    },
    /// Two children sharing the space.
    Split {
        /// Direction.
        axis: Axis,
        /// Share of the first child in per mille (1..=999).
        ratio: u16,
        /// Left or top child.
        first: Box<Layout>,
        /// Right or bottom child.
        second: Box<Layout>,
    },
}

impl Layout {
    /// All `sessions` along `axis` with equal shares, as a balanced tree (depth ⌈log2 n⌉, so even
    /// thousands of sessions stay far inside [`MAX_LAYOUT_DEPTH`]; every ratio is 333 to 500).
    /// `None` when `sessions` is empty or the tree could not satisfy [`Layout::validate`].
    ///
    /// ```
    /// use ferrum_project::{Axis, Layout, SessionKey};
    /// let layout = Layout::equal(&[SessionKey(1), SessionKey(2)], Axis::Horizontal).unwrap();
    /// assert_eq!(layout.sessions(), [SessionKey(1), SessionKey(2)]);
    /// ```
    pub fn equal(sessions: &[SessionKey], axis: Axis) -> Option<Self> {
        Self::balanced(sessions, axis, 0)
    }

    fn balanced(sessions: &[SessionKey], axis: Axis, depth: usize) -> Option<Self> {
        match sessions {
            [] => None,
            [session] => Some(Self::Pane { session: *session }),
            _ if depth >= MAX_LAYOUT_DEPTH => None,
            _ => {
                let mid = sessions.len() / 2;
                let (first, second) = sessions.split_at(mid);
                let share = u64::try_from(mid).ok()?.checked_mul(u64::from(RATIO_SCALE))?;
                let ratio = u16::try_from(share / u64::try_from(sessions.len()).ok()?).ok()?;
                Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(Self::balanced(first, axis, depth.saturating_add(1))?),
                    second: Box::new(Self::balanced(second, axis, depth.saturating_add(1))?),
                })
            }
        }
    }

    /// Sessions shown, left to right / top to bottom.
    pub fn sessions(&self) -> Vec<SessionKey> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            match node {
                Self::Pane { session } => out.push(*session),
                Self::Split { first, second, .. } => {
                    stack.push(second);
                    stack.push(first);
                }
            }
        }
        out
    }

    /// Checks depth, ratios, and that every pane shows a distinct session from `known`.
    ///
    /// # Errors
    /// [`ProjectError::LayoutTooDeep`], [`ProjectError::BadRatio`],
    /// [`ProjectError::UnknownLayoutSession`], [`ProjectError::DuplicateLayoutSession`].
    pub fn validate(&self, known: &HashSet<SessionKey>) -> Result<(), ProjectError> {
        let mut seen = HashSet::new();
        let mut stack = vec![(self, 0_usize)];
        while let Some((node, depth)) = stack.pop() {
            match node {
                Self::Pane { session } => {
                    if !known.contains(session) {
                        return Err(ProjectError::UnknownLayoutSession(*session));
                    }
                    if !seen.insert(*session) {
                        return Err(ProjectError::DuplicateLayoutSession(*session));
                    }
                }
                Self::Split { ratio, first, second, .. } => {
                    if depth >= MAX_LAYOUT_DEPTH {
                        return Err(ProjectError::LayoutTooDeep { max: MAX_LAYOUT_DEPTH });
                    }
                    if !(1..RATIO_SCALE).contains(ratio) {
                        return Err(ProjectError::BadRatio(*ratio));
                    }
                    stack.push((second, depth.saturating_add(1)));
                    stack.push((first, depth.saturating_add(1)));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: u64) -> Vec<SessionKey> {
        (1..=n).map(SessionKey).collect()
    }

    #[test]
    fn equal_layouts_keep_order_and_share_space_evenly() {
        assert_eq!(Layout::equal(&[], Axis::Vertical), None);
        let three = Layout::equal(&keys(3), Axis::Vertical).unwrap();
        assert_eq!(three.sessions(), keys(3));
        let Layout::Split { ratio, second, .. } = &three else { panic!("not a split") };
        assert_eq!(*ratio, 333);
        assert!(matches!(**second, Layout::Split { ratio: 500, .. }));
    }

    #[test]
    fn equal_layouts_of_many_sessions_are_valid() {
        // Past 65 536 the old builder overflowed; Miri runs the small sizes only (an interpreted
        // 70 000-pane tree takes about an hour there).
        let sizes: &[u64] = if cfg!(miri) { &[34, 1_002] } else { &[34, 1_002, 70_000] };
        for &n in sizes {
            let all = keys(n);
            let layout = Layout::equal(&all, Axis::Horizontal).unwrap();
            assert_eq!(layout.sessions(), all, "{n}");
            let known: HashSet<_> = all.into_iter().collect();
            assert_eq!(layout.validate(&known), Ok(()), "{n}");
        }
    }

    #[test]
    fn validation_rejects_unknown_duplicate_bad_ratio_and_deep_trees() {
        let known: HashSet<_> = keys(2).into_iter().collect();
        assert_eq!(Layout::equal(&keys(2), Axis::Horizontal).unwrap().validate(&known), Ok(()));
        let unknown = Layout::Pane { session: SessionKey(9) };
        assert_eq!(
            unknown.validate(&known),
            Err(ProjectError::UnknownLayoutSession(SessionKey(9)))
        );
        let twice = Layout::equal(&[SessionKey(1), SessionKey(1)], Axis::Horizontal).unwrap();
        assert_eq!(
            twice.validate(&known),
            Err(ProjectError::DuplicateLayoutSession(SessionKey(1)))
        );
        let pane = |k| Box::new(Layout::Pane { session: SessionKey(k) });
        for ratio in [0, RATIO_SCALE] {
            let bad =
                Layout::Split { axis: Axis::Vertical, ratio, first: pane(1), second: pane(2) };
            assert_eq!(bad.validate(&known), Err(ProjectError::BadRatio(ratio)));
        }
        let deep_keys = keys(u64::try_from(MAX_LAYOUT_DEPTH).unwrap() + 2);
        let mut deep = Layout::Pane { session: SessionKey(1) };
        for &session in deep_keys.iter().skip(1) {
            deep = Layout::Split {
                axis: Axis::Vertical,
                ratio: 500,
                first: pane(session.0),
                second: Box::new(deep),
            };
        }
        let all: HashSet<_> = deep_keys.into_iter().collect();
        assert!(matches!(deep.validate(&all), Err(ProjectError::LayoutTooDeep { .. })));
    }
}
