//! The authoritative project store (`docs/02` §8, FR-PROJ-001/005): every project in sidebar
//! order, each persisted as `data_dir/ferrum/projects/<id>.toml`, and the sidebar order in
//! `data_dir/ferrum/project-order.toml`.
//!
//! Files are read once at start; an unusable one is set aside as `*.bad-<time>` by
//! `ferrum_config::store` and skipped. Every change is written by the worker with an atomic
//! write. A session's key in its project file is its daemon session id, so ids are stable across
//! daemon restarts (FR-SESS-008).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ferrum_config::store::{self, Source};
use ferrum_config::{ConfigError, Schema};
use ferrum_project::{PROJECT_SCHEMA, PROJECTS_DIR, Project, ProjectId, ProjectParts};
use serde::{Deserialize, Serialize};

/// File of the sidebar order.
const ORDER_FILE: &str = "project-order.toml";
/// Extension of project files.
const PROJECT_EXTENSION: &str = "toml";
/// Schema of the order file.
pub(crate) const ORDER_SCHEMA: Schema =
    Schema { name: "project-order", current: 1, migrations: &[] };

/// The order file: project ids, first at the top of the sidebar.
#[derive(Debug, Default, Serialize, Deserialize)]
struct OrderFile {
    #[serde(default)]
    order: Vec<ProjectId>,
}

/// All projects.
#[derive(Debug, Default)]
pub(crate) struct Projects {
    dir: PathBuf,
    order_path: PathBuf,
    order: Vec<ProjectId>,
    items: HashMap<ProjectId, Project>,
    /// Files found unusable at start and set aside: `(file, backup)`, for the UI to show
    /// (`docs/02` §8 and §10).
    pub(crate) set_aside: Vec<(PathBuf, PathBuf)>,
}

/// A file to write.
pub(crate) struct Write {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

impl Projects {
    /// Reads the projects under `data_dir`.
    ///
    /// # Errors
    /// [`ConfigError`] when the directory cannot be listed (a missing one is empty).
    pub(crate) fn load(data_dir: &Path) -> Result<Self, ConfigError> {
        let dir = data_dir.join(PROJECTS_DIR);
        let order_path = data_dir.join(ORDER_FILE);
        let mut items = HashMap::new();
        let mut set_aside = Vec::new();
        for path in project_files(&dir)? {
            let loaded = load::<Option<Project>>(&path, &PROJECT_SCHEMA, &mut set_aside);
            if let Some(project) = loaded.flatten() {
                items.insert(project.id(), project);
            }
        }
        let saved = load::<OrderFile>(&order_path, &ORDER_SCHEMA, &mut set_aside)
            .map(|file| file.order)
            .unwrap_or_default();
        let mut order: Vec<ProjectId> =
            saved.into_iter().filter(|id| items.contains_key(id)).collect();
        let mut rest: Vec<ProjectId> =
            items.keys().filter(|id| !order.contains(id)).copied().collect();
        rest.sort_by_key(|id| items.get(id).map(|project| project.name().to_owned()));
        order.append(&mut rest);
        Ok(Self { dir, order_path, order, items, set_aside })
    }

    /// The projects in sidebar order.
    pub(crate) fn ordered(&self) -> impl Iterator<Item = &Project> {
        self.order.iter().filter_map(|id| self.items.get(id))
    }

    pub(crate) fn get(&self, id: ProjectId) -> Option<&Project> {
        self.items.get(&id)
    }

    /// The remembered working directory of session `key` in `project`.
    pub(crate) fn session_cwd(&self, project: ProjectId, key: u64) -> Option<&Path> {
        let entry = self.get(project)?.sessions().iter().find(|entry| entry.key.0 == key)?;
        entry.cwd.as_deref()
    }

    /// The project holding the session with key `key`.
    pub(crate) fn owner_of(&self, key: u64) -> Option<ProjectId> {
        self.ordered().find(|p| p.sessions().iter().any(|s| s.key.0 == key)).map(Project::id)
    }

    /// Adds a new project at the end of the sidebar.
    pub(crate) fn insert(&mut self, project: Project) -> Vec<Write> {
        let id = project.id();
        self.items.insert(id, project);
        self.order.push(id);
        [self.file_of(id), Some(self.order_file())].into_iter().flatten().collect()
    }

    /// Applies `change` to a project; nothing changes when it fails.
    ///
    /// # Errors
    /// The error `change` returns, or the validation error of the changed parts.
    pub(crate) fn update<E: From<ferrum_project::ProjectError>>(
        &mut self,
        id: ProjectId,
        change: impl FnOnce(&mut ProjectParts) -> Result<(), E>,
    ) -> Result<Option<Write>, E> {
        let Some(project) = self.items.get(&id) else { return Ok(None) };
        let mut parts = project.clone().into_parts();
        change(&mut parts)?;
        let updated = Project::try_from(parts)?;
        self.items.insert(id, updated);
        Ok(self.file_of(id))
    }

    /// Moves a project to `index` in the sidebar (clamped).
    pub(crate) fn reorder(&mut self, id: ProjectId, index: usize) -> Option<Write> {
        let from = self.order.iter().position(|&known| known == id)?;
        self.order.remove(from);
        self.order.insert(index.min(self.order.len()), id);
        Some(self.order_file())
    }

    fn file_of(&self, id: ProjectId) -> Option<Write> {
        let project = self.items.get(&id)?;
        let path = self.dir.join(format!("{}.{PROJECT_EXTENSION}", id.file_name()));
        match store::to_toml_string(project, &PROJECT_SCHEMA) {
            Ok(text) => Some(Write { path, text }),
            Err(error) => {
                tracing::error!(kind = error.kind(), "a project could not be serialized");
                None
            }
        }
    }

    fn order_file(&self) -> Write {
        let order = OrderFile { order: self.order.clone() };
        // A list of UUIDs always serializes.
        let text = store::to_toml_string(&order, &ORDER_SCHEMA).unwrap_or_default();
        Write { path: self.order_path.clone(), text }
    }
}

/// Reads one file; an unusable one is set aside (recorded in `set_aside`) and gives the default.
fn load<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
    schema: &Schema,
    set_aside: &mut Vec<(PathBuf, PathBuf)>,
) -> Option<T> {
    match store::load_or_recover::<T>(path, schema, SystemTime::now()) {
        Ok(loaded) => {
            if let Source::Recovered { backup, .. } = loaded.source {
                tracing::error!("an unusable project file was set aside");
                set_aside.push((path.to_path_buf(), backup));
            }
            Some(loaded.value)
        }
        Err(error) => {
            tracing::error!(kind = error.kind(), "a project file was skipped");
            None
        }
    }
}

/// The project files in `dir` (a missing directory holds none).
fn project_files(dir: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(ConfigError::Io { path: dir.to_path_buf(), source: error }),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == PROJECT_EXTENSION))
        .collect();
    files.sort();
    Ok(files)
}
