//! UI scripts about projects and layouts (see `ui.rs`):
//!
//! | Script | Does | Prints |
//! |---|---|---|
//! | `layout` | a project with two sessions side by side; the first moves to `<data>/work` (OSC 7) | `LAYOUT <first> <second> <layout>`, then `SETUP-DONE` |
//! | `inspect` | reads the state after `Welcome`; waits for the first session to start in `<data>/work` | `LAYOUT ...`, `RUNNING <n>`, `CWD-OK`, then `CHECK-DONE` |

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    dead_code,
    unreachable_pub,
    reason = "test support"
)]

use std::io::Write;
use std::path::Path;

use ferrum_proto::{
    Axis, ClientMessage, DaemonMessage, Layout, LayoutNode, List, PerMille, ProjectEvent,
    SessionEvent, SessionStatus,
};

use super::harness::Client;

/// A path as text, without Windows' verbatim prefix, for comparison.
fn normalized(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("//?/").map_or(text.clone(), str::to_owned)
}

/// The directory the first session moves to.
fn work_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("work")
}

pub fn layout(client: &mut Client, data_dir: &Path, out: &mut impl Write) {
    let project = client.create_project(data_dir);
    let first = client.create_session(project);
    let second = client.create_session(project);
    let work = work_dir(data_dir);
    std::fs::create_dir_all(&work).unwrap();
    client.attach(first, 0);
    client.wait_text(first, "READY", |text| text.contains("READY"));
    let line = format!("{} {}", "cd", work.display());
    client.type_line(first, &line);
    client.wait_text(first, "CD-OK", |text| text.contains("CD-OK"));
    let split = LayoutNode::Split { axis: Axis::Horizontal, ratio: PerMille::HALF };
    let nodes = vec![split, LayoutNode::Pane(first), LayoutNode::Pane(second)];
    let layout = Layout(List::new(nodes).unwrap());
    client.send(&ClientMessage::SaveLayout { project_id: project, layout: layout.clone() });
    client.wait_for("layout", |m| matches!(m, DaemonMessage::LayoutEvent { .. }));
    writeln!(out, "LAYOUT {first:?} {second:?} {layout:?}").unwrap();
    writeln!(out, "SETUP-DONE").unwrap();
}

pub fn inspect(client: &mut Client, data_dir: &Path, out: &mut impl Write) {
    let mut projects = Vec::new();
    let mut running = 0;
    for message in &client.state {
        match message {
            DaemonMessage::ProjectEvent(ProjectEvent::Created(info)) => projects.push(info.clone()),
            DaemonMessage::SessionEvent { event: SessionEvent::Created(info), .. }
                if matches!(info.status, SessionStatus::Running) =>
            {
                running += 1;
            }
            _ => {}
        }
    }
    let info = &projects[0];
    let sessions = info.sessions.as_slice();
    writeln!(out, "LAYOUT {:?} {:?} {:?}", sessions[0], sessions[1], info.layout).unwrap();
    writeln!(out, "RUNNING {running}").unwrap();
    client.attach(sessions[0], 0);
    // FR-SESS-008: the respawned child starts in the remembered directory, compared in full in
    // its canonical form (the OS may report `/private/var` for `/var` on macOS).
    let expected = normalized(&std::fs::canonicalize(work_dir(data_dir)).unwrap());
    client.wait_text(sessions[0], "the remembered directory", |text| {
        // A long path wraps: compare the rows joined.
        let flat: String = text.lines().map(str::trim_end).collect();
        let reported = flat.split("READY ").nth(1).unwrap_or_default();
        let reported = std::fs::canonicalize(reported.trim()).map(|path| normalized(&path));
        reported.is_ok_and(|reported| reported == expected)
    });
    writeln!(out, "CWD-OK").unwrap();
    writeln!(out, "CHECK-DONE").unwrap();
}
