//! Projects persisted through the versioned store (`FR-PROJ-001`, `FR-PROJ-005`, `FR-PROJ-007`,
//! `FR-DATA-001`): round trip, defaults, and validation of what a file may contain.

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use ferrum_config::{Rgb, Source, store};
use ferrum_project::{
    Axis, Layout, PROJECT_SCHEMA, PROJECTS_DIR, Project, ProjectError, ProjectId, Recipe,
    SessionEntry, SessionKey,
};
use ferrum_testkit::temp::TempDir;

fn two_root_project() -> Project {
    let mut project =
        Project::new(ProjectId(7), vec![PathBuf::from("/w/front"), PathBuf::from("/w/back")]);
    project.icon = Some("◼".to_owned());
    project.color = Rgb { r: 0x22, g: 0x88, b: 0xcc };
    let mut back = SessionEntry::new(SessionKey(2), 1);
    back.name = Some("api".to_owned());
    back.cwd = Some(PathBuf::from("/w/back/src"));
    project.sessions = vec![SessionEntry::new(SessionKey(1), 0), back];
    project.layout = Layout::equal(&[SessionKey(1), SessionKey(2)], Axis::Horizontal);
    project.recipe = Some(Recipe::front_and_back(2));
    project
}

#[test]
fn project_round_trips_through_its_file() {
    let dir = TempDir::new("project-file").unwrap();
    let project = two_root_project();
    assert_eq!(project.validate(), Ok(()));
    assert_eq!(project.name, "front", "named after the first root");
    let path = dir.join(PROJECTS_DIR).join(project.id.file_name());
    store::save(&path, &project, &PROJECT_SCHEMA).unwrap();
    let loaded =
        store::load_or_recover::<Option<Project>>(&path, &PROJECT_SCHEMA, UNIX_EPOCH).unwrap();
    assert!(matches!(loaded.source, Source::File));
    assert_eq!(loaded.value, Some(project));
}

#[test]
fn minimal_file_gets_defaults() {
    let text = "schema_version = 1\nid = 3\nname = \"Scratch\"\nroots = [\"/tmp\"]\n";
    let (project, _) = store::parse::<Project>(text, &PROJECT_SCHEMA).unwrap();
    assert_eq!(project.validate(), Ok(()));
    assert_eq!(project.color, ferrum_project::DEFAULT_COLOR);
    assert!(project.sessions.is_empty() && project.layout.is_none() && project.recipe.is_none());
    assert_eq!(project.default_root(), Some(PathBuf::from("/tmp").as_path()));
}

/// A change that makes a valid project invalid.
type Breakage = fn(&mut Project);

#[test]
fn invalid_projects_are_reported() {
    let cases: [(&str, Breakage); 6] = [
        ("empty name", |p| p.name = "  ".to_owned()),
        ("no roots", |p| p.roots.clear()),
        ("same root twice", |p| p.roots = vec![PathBuf::from("/a"), PathBuf::from("/a")]),
        ("duplicate session", |p| p.sessions.push(SessionEntry::new(SessionKey(1), 0))),
        ("unknown root", |p| p.sessions.push(SessionEntry::new(SessionKey(3), 5))),
        ("layout names a missing session", |p| {
            p.layout = Some(Layout::Pane { session: SessionKey(9) });
        }),
    ];
    for (what, break_it) in cases {
        let mut project = two_root_project();
        break_it(&mut project);
        assert!(project.validate().is_err(), "{what}");
    }
    let mut bad_icon = two_root_project();
    bad_icon.icon = Some(String::new());
    assert!(matches!(bad_icon.validate(), Err(ProjectError::BadIcon { .. })));
}

#[test]
fn malformed_project_file_is_a_config_error() {
    for text in [
        "id = 1\nname = \"x\"\nroots = [\"/a\"]\ncolor = \"orange\"",
        "id = \"x\"\nname = \"x\"\nroots = []",
    ] {
        assert!(store::parse::<Project>(text, &PROJECT_SCHEMA).is_err(), "{text}");
    }
    let unversioned_ok = "id = 1\nname = \"x\"\nroots = [\"/a\"]";
    let err = store::parse::<Project>(unversioned_ok, &PROJECT_SCHEMA).unwrap_err();
    assert!(
        err.to_string().contains("older"),
        "files without schema_version predate projects: {err}"
    );
}
