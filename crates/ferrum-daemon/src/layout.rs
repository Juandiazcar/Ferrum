//! Layouts between the project model (a tree of [`SessionKey`]s) and the protocol (a flat
//! pre-order list of [`SessionId`]s): the daemon's session ids are its project keys.
//!
//! Converting untrusted protocol input never recurses: the list is read with an explicit stack,
//! and the tree shape and depth are then checked by [`Layout::validate`] (`FR-PROJ-005`).

use std::collections::HashSet;

use ferrum_project::{Axis, Layout, MAX_LAYOUT_DEPTH, SessionKey};
use ferrum_proto::{self as proto, LayoutNode, List, PerMille, SessionId};

/// The protocol form of a project layout (`None` is an empty layout).
pub(crate) fn to_proto(layout: Option<&Layout>) -> proto::Layout {
    let mut nodes = Vec::new();
    let mut stack: Vec<&Layout> = layout.into_iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Layout::Pane { session } => nodes.push(LayoutNode::Pane(SessionId(session.0))),
            Layout::Split { axis, ratio, first, second } => {
                let axis = match axis {
                    Axis::Horizontal => proto::Axis::Horizontal,
                    Axis::Vertical => proto::Axis::Vertical,
                };
                let ratio = PerMille::try_from(*ratio).unwrap_or(PerMille::HALF);
                nodes.push(LayoutNode::Split { axis, ratio });
                stack.push(second);
                stack.push(first);
            }
        }
    }
    // A valid project layout has at most one pane per session, far fewer than the list cap.
    proto::Layout(List::new(nodes).unwrap_or_default())
}

/// A pending split while reading the pre-order list.
struct Open {
    axis: Axis,
    ratio: u16,
    first: Option<Layout>,
}

/// The project layout of a protocol layout whose panes are all in `known`; `None` when the list
/// is not a well-formed tree within [`MAX_LAYOUT_DEPTH`] or names an unknown session.
pub(crate) fn from_proto(
    layout: &proto::Layout,
    known: &HashSet<SessionKey>,
) -> Result<Option<Layout>, InvalidLayout> {
    let nodes = layout.0.as_slice();
    if nodes.is_empty() {
        return Ok(None);
    }
    let tree = tree(nodes).ok_or(InvalidLayout)?;
    tree.validate(known).map_err(|_| InvalidLayout)?;
    Ok(Some(tree))
}

/// A layout list that is not a valid tree of known sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InvalidLayout;

/// The tree of a non-empty pre-order list, without the session checks.
fn tree(nodes: &[LayoutNode]) -> Option<Layout> {
    let mut open: Vec<Open> = Vec::new();
    let mut done: Option<Layout> = None;
    for node in nodes {
        if done.is_some() {
            return None; // nodes after a complete tree
        }
        let mut finished = match *node {
            LayoutNode::Pane(id) => Layout::Pane { session: SessionKey(id.0) },
            LayoutNode::Split { axis, ratio } => {
                if open.len() >= MAX_LAYOUT_DEPTH {
                    return None;
                }
                let axis =
                    if axis == proto::Axis::Vertical { Axis::Vertical } else { Axis::Horizontal };
                open.push(Open { axis, ratio: ratio.get(), first: None });
                continue;
            }
            _ => return None,
        };
        // Close every split whose second child just completed.
        loop {
            let Some(top) = open.last_mut() else {
                done = Some(finished);
                break;
            };
            let Some(first) = top.first.take() else {
                top.first = Some(finished);
                break;
            };
            let Open { axis, ratio, .. } = open.pop()?;
            finished =
                Layout::Split { axis, ratio, first: Box::new(first), second: Box::new(finished) };
        }
    }
    done
}

/// The layout without the pane of `key`: its sibling takes the place of their split. `None` when
/// the layout was that pane alone. Recursion is bounded by [`MAX_LAYOUT_DEPTH`] (a project's
/// layout is validated).
pub(crate) fn without(layout: Layout, key: SessionKey) -> Option<Layout> {
    match layout {
        Layout::Pane { session } if session == key => None,
        Layout::Split { axis, ratio, first, second } => {
            match (without(*first, key), without(*second, key)) {
                (Some(first), Some(second)) => Some(Layout::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            }
        }
        pane @ Layout::Pane { .. } => Some(pane),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(ids: &[u64]) -> HashSet<SessionKey> {
        ids.iter().map(|&id| SessionKey(id)).collect()
    }

    #[test]
    fn layouts_round_trip_and_bad_lists_are_refused() {
        let layout = Layout::equal(&[SessionKey(1), SessionKey(2), SessionKey(3)], Axis::Vertical);
        let wire = to_proto(layout.as_ref());
        assert_eq!(from_proto(&wire, &keys(&[1, 2, 3])), Ok(layout));
        assert_eq!(from_proto(&wire, &keys(&[1, 2])), Err(InvalidLayout), "unknown session");
        let split = LayoutNode::Split { axis: proto::Axis::Horizontal, ratio: PerMille::HALF };
        let pane = |id| LayoutNode::Pane(SessionId(id));
        let bad = |nodes: Vec<LayoutNode>| proto::Layout(List::new(nodes).unwrap());
        assert_eq!(
            from_proto(&bad(vec![split, pane(1)]), &keys(&[1])),
            Err(InvalidLayout),
            "missing child"
        );
        assert_eq!(
            from_proto(&bad(vec![pane(1), pane(2)]), &keys(&[1, 2])),
            Err(InvalidLayout),
            "two roots"
        );
        assert_eq!(
            from_proto(&bad(vec![split; MAX_LAYOUT_DEPTH + 1]), &keys(&[])),
            Err(InvalidLayout)
        );
        assert_eq!(from_proto(&proto::Layout::default(), &keys(&[])), Ok(None));
        let three =
            Layout::equal(&[SessionKey(1), SessionKey(2), SessionKey(3)], Axis::Vertical).unwrap();
        let two = without(three, SessionKey(2)).unwrap();
        assert_eq!(two.sessions(), [SessionKey(1), SessionKey(3)]);
        assert_eq!(without(Layout::Pane { session: SessionKey(1) }, SessionKey(1)), None);
    }
}
