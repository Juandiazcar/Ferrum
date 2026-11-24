//! A checkout of another revision of this repository in a git worktree, for the gates that
//! compare with the previous commit on the same machine (`docs/08` §8).

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::tools;

/// A checkout of `rev` in a git worktree, removed when dropped.
pub(crate) struct Worktree {
    repo: PathBuf,
    pub(crate) path: PathBuf,
}

impl Worktree {
    pub(crate) fn add(repo: &Path, path: PathBuf, rev: &str) -> Result<Self> {
        let repo_arg = repo.to_string_lossy();
        let path_arg = path.to_string_lossy();
        // Leftovers of an interrupted run.
        tools::run(&["git", "-C", &repo_arg, "worktree", "remove", "--force", &path_arg], &[]).ok();
        anyhow::ensure!(
            tools::run(
                &["git", "-C", &repo_arg, "worktree", "add", "--detach", &path_arg, rev],
                &[]
            )?,
            "cannot check out `{rev}` (is the history fetched, e.g. `fetch-depth: 2`?)"
        );
        Ok(Self { repo: repo.to_path_buf(), path })
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        let (repo, path) = (self.repo.to_string_lossy(), self.path.to_string_lossy());
        // Best effort: a leftover worktree under target/ is removed by the next run.
        tools::run(&["git", "-C", &repo, "worktree", "remove", "--force", &path], &[]).ok();
    }
}
