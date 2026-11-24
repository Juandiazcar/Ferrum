//! Projects persisted through the versioned store (`FR-PROJ-001`, `FR-PROJ-005`, `FR-PROJ-007`,
//! `FR-DATA-001`): round trip, defaults, and validation at the parse boundary.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use ferrum_config::{ConfigError, MaxRoots, ProjectLimits, Rgb, Source, store};
use ferrum_project::{
    Axis, Layout, PROJECT_SCHEMA, PROJECTS_DIR, Project, ProjectError, ProjectId, ProjectParts,
    Recipe, SessionEntry, SessionKey,
};
use ferrum_testkit::temp::TempDir;

/// An absolute path on every OS.
fn abs(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn two_root_parts() -> ProjectParts {
    let roots = vec![abs("front"), abs("back")];
    let mut parts = Project::new(ProjectId::new_random(), roots.clone()).unwrap().into_parts();
    parts.icon = Some("◼".to_owned());
    parts.color = Rgb { r: 0x22, g: 0x88, b: 0xcc };
    let mut back = SessionEntry::new(SessionKey(2), 1);
    back.name = Some("api".to_owned());
    back.cwd = Some(abs("back/src"));
    parts.sessions = vec![SessionEntry::new(SessionKey(1), 0), back];
    parts.layout = Layout::equal(&[SessionKey(1), SessionKey(2)], Axis::Horizontal);
    parts.recipe = Recipe::front_and_back(&roots);
    parts
}

#[test]
fn project_round_trips_through_its_file() {
    let dir = TempDir::new("project-file").unwrap();
    let project = Project::try_from(two_root_parts()).unwrap();
    assert_eq!(project.name(), "front", "named after the first root");
    let path = dir.join(PROJECTS_DIR).join(project.id().file_name());
    store::save(&path, &project, &PROJECT_SCHEMA).unwrap();
    let loaded =
        store::load_or_recover::<Option<Project>>(&path, &PROJECT_SCHEMA, UNIX_EPOCH).unwrap();
    assert!(matches!(loaded.source, Source::File));
    assert_eq!(loaded.value, Some(project));
}

#[test]
fn ids_are_random_uuids_and_name_the_file() {
    let (a, b) = (ProjectId::new_random(), ProjectId::new_random());
    assert_ne!(a, b);
    assert_eq!(a.file_name(), format!("{a}.toml"));
    assert_eq!(a.to_string().len(), 36, "hyphenated UUID");
}

#[test]
fn minimal_file_gets_defaults() {
    let root = abs("scratch");
    let text = format!(
        "schema_version = 1\nid = \"67e55044-10b1-426f-9247-bb680e5fe0c8\"\nname = \"Scratch\"\n\
         roots = [{}]\n",
        toml::Value::String(root.to_string_lossy().into_owned())
    );
    let (project, _) = store::parse::<Project>(&text, &PROJECT_SCHEMA).unwrap();
    assert_eq!(project.color(), ferrum_project::DEFAULT_COLOR);
    assert!(project.sessions().is_empty() && project.layout().is_none());
    assert_eq!(project.default_root(), root.as_path());
}

/// A change that makes valid parts invalid.
type Breakage = fn(&mut ProjectParts);

#[test]
fn invalid_parts_never_become_a_project() {
    let cases: [(&str, Breakage); 12] = [
        ("empty name", |p| p.name = "  ".to_owned()),
        ("control in name", |p| p.name = "a\u{7}".to_owned()),
        ("no roots", |p| p.roots.clear()),
        ("relative root", |p| p.roots = vec![PathBuf::from("relative/dir")]),
        ("same root twice", |p| p.roots = vec![abs("a"), abs("a")]),
        ("duplicate session", |p| p.sessions.push(SessionEntry::new(SessionKey(1), 0))),
        ("unknown root", |p| p.sessions.push(SessionEntry::new(SessionKey(3), 5))),
        ("empty profile", |p| p.sessions[0].profile = Some(String::new())),
        ("relative cwd", |p| p.sessions[0].cwd = Some(PathBuf::from("src"))),
        ("two glyphs as icon", |p| p.icon = Some("ab".to_owned())),
        ("empty icon", |p| p.icon = Some(String::new())),
        ("layout names a missing session", |p| {
            p.layout = Some(Layout::Pane { session: SessionKey(9) });
        }),
    ];
    for (what, break_it) in cases {
        let mut parts = two_root_parts();
        break_it(&mut parts);
        assert!(Project::try_from(parts).is_err(), "{what}");
    }
    let many_roots = (0..=MaxRoots::MAX).map(|n| abs(&format!("r{n}"))).collect();
    assert!(matches!(
        Project::new(ProjectId::new_random(), many_roots),
        Err(ProjectError::BadRoots { .. })
    ));
    assert!(matches!(
        Project::new(ProjectId::new_random(), Vec::new()),
        Err(ProjectError::BadRoots { .. })
    ));
}

#[test]
fn an_invalid_file_is_rejected_while_parsing() {
    let mut parts = two_root_parts();
    parts.roots = vec![PathBuf::from("relative")];
    parts.sessions.clear();
    parts.layout = None;
    parts.recipe = None;
    let table = toml::Table::try_from(&parts).unwrap();
    let text = toml::to_string(&PROJECT_SCHEMA.stamp(table)).unwrap();
    let err = store::parse::<Project>(&text, &PROJECT_SCHEMA).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_)), "{err}");
    let unversioned = "id = \"67e55044-10b1-426f-9247-bb680e5fe0c8\"\nname = \"x\"\nroots = []";
    let err = store::parse::<Project>(unversioned, &PROJECT_SCHEMA).unwrap_err();
    assert!(matches!(err, ConfigError::TooOld { .. }), "files without schema_version: {err}");
}

#[test]
fn an_icon_is_any_single_glyph_up_to_the_longest_emoji_sequences() {
    // kiss: woman, man, medium skin tone, dark skin tone (35 bytes, one grapheme cluster).
    let kiss =
        "\u{1f469}\u{1f3fd}\u{200d}\u{2764}\u{fe0f}\u{200d}\u{1f48b}\u{200d}\u{1f468}\u{1f3ff}";
    assert_eq!(kiss.len(), 35);
    let mut parts = two_root_parts();
    parts.icon = Some(kiss.to_owned());
    assert!(Project::try_from(parts).is_ok());
}

#[test]
fn user_limits_from_the_settings_apply_on_edit() {
    let project = Project::try_from(two_root_parts()).unwrap();
    assert_eq!(project.check_limits(&ProjectLimits::default()), Ok(()));
    let mut tight = ProjectLimits::default();
    tight.max_roots = MaxRoots::new(1).unwrap();
    assert!(matches!(project.check_limits(&tight), Err(ProjectError::BadRoots { max: 1 })));
}
