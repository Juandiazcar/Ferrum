//! The `.git/HEAD` reader against fixture repositories created in temporary directories
//! (`FR-PROJ-004`, `docs/12` WP-04 exit): branches, detached heads, linked worktrees and
//! submodules, with the on-disk layout git writes. No git binary and no real user directory.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::fs;
use std::path::{Path, PathBuf};

use ferrum_project::{GitError, GitHead, Head, MAX_GIT_FILE_BYTES, read_head};
use ferrum_testkit::temp::TempDir;

const SHA1: &str = "3f786850e387550fdab836ed7e6dc881de23001b";

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// `<root>/.git/HEAD` with `head`, like `git init` followed by a checkout.
fn repo(root: &Path, head: &str) {
    write(&root.join(".git/HEAD"), head);
    fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
}

fn head_of(dir: &Path) -> GitHead {
    read_head(dir).unwrap().unwrap()
}

fn branch(name: &str) -> Head {
    Head::Branch(name.to_owned())
}

#[test]
fn branch_is_found_from_the_root_and_from_nested_directories() {
    let tmp = TempDir::new("git-branch").unwrap();
    let root = tmp.join("repo");
    repo(&root, "ref: refs/heads/main\n");
    let nested = root.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    for start in [root.clone(), nested] {
        let found = head_of(&start);
        assert_eq!(found.head, branch("main"));
        assert_eq!((found.work_tree, found.git_dir), (root.clone(), root.join(".git")));
    }
}

#[test]
fn branch_names_with_slashes_and_unborn_branches_are_read() {
    let tmp = TempDir::new("git-branch").unwrap();
    repo(tmp.path(), "ref: refs/heads/feature/login\r\n");
    assert_eq!(head_of(tmp.path()).head, branch("feature/login"));
    repo(tmp.path(), "ref: refs/heads/never-committed\n");
    assert_eq!(head_of(tmp.path()).head, branch("never-committed"));
}

#[test]
fn detached_heads_are_reported_with_their_commit() {
    let tmp = TempDir::new("git-detached").unwrap();
    repo(tmp.path(), &format!("{SHA1}\n"));
    assert_eq!(head_of(tmp.path()).head, Head::Detached(SHA1.to_owned()));
    let sha256 = "0f".repeat(32);
    repo(tmp.path(), &format!("{sha256}\n"));
    assert_eq!(head_of(tmp.path()).head, Head::Detached(sha256));
}

#[test]
fn symbolic_refs_outside_heads_are_kept_whole() {
    let tmp = TempDir::new("git-ref").unwrap();
    repo(tmp.path(), "ref: refs/remotes/origin/main\n");
    assert_eq!(head_of(tmp.path()).head, Head::OtherRef("refs/remotes/origin/main".to_owned()));
}

#[test]
fn linked_worktrees_read_their_own_head() {
    let tmp = TempDir::new("git-worktree").unwrap();
    let main = tmp.join("main");
    repo(&main, "ref: refs/heads/main\n");
    let admin = main.join(".git/worktrees/topic");
    write(&admin.join("HEAD"), "ref: refs/heads/topic\n");
    write(&admin.join("commondir"), "../..\n");
    // `git worktree add` writes an absolute gitdir; a relative one (worktree.useRelativePaths) is
    // relative to the worktree directory.
    let absolute = tmp.join("topic-abs");
    write(&absolute.join(".git"), &format!("gitdir: {}\n", admin.display()));
    let relative = tmp.join("topic-rel");
    write(&relative.join(".git"), "gitdir: ../main/.git/worktrees/topic\n");
    for worktree in [absolute, relative] {
        let found = head_of(&worktree.join("sub"));
        assert_eq!(found.head, branch("topic"));
        assert_eq!(found.work_tree, worktree);
        assert_eq!(fs::canonicalize(&found.git_dir).unwrap(), fs::canonicalize(&admin).unwrap());
    }
    assert_eq!(head_of(&main).head, branch("main"), "the main worktree is unaffected");
}

#[test]
fn submodules_read_their_own_head_not_the_superproject() {
    let tmp = TempDir::new("git-submodule").unwrap();
    let superproject = tmp.join("super");
    repo(&superproject, "ref: refs/heads/main\n");
    write(&superproject.join(".git/modules/vendor/lib/HEAD"), &format!("{SHA1}\n"));
    let module = superproject.join("vendor/lib");
    write(&module.join(".git"), "gitdir: ../../.git/modules/vendor/lib\n");
    fs::create_dir_all(module.join("src")).unwrap();
    let found = head_of(&module.join("src"));
    assert_eq!(found.head, Head::Detached(SHA1.to_owned()));
    assert_eq!(found.work_tree, module);
    assert_eq!(head_of(&superproject.join("vendor")).head, branch("main"));
}

#[test]
fn directories_outside_any_repository_have_no_head() {
    let tmp = TempDir::new("git-none").unwrap();
    // The temporary directory itself may sit inside a repository (e.g. TMPDIR under a checkout):
    // whatever the ambient answer is, nothing inside `tmp` may be reported.
    let ambient = read_head(tmp.path()).unwrap();
    let plain: PathBuf = tmp.join("plain/dir");
    fs::create_dir_all(&plain).unwrap();
    let found = read_head(&plain).unwrap();
    assert_eq!(found, ambient);
    assert!(found.is_none_or(|head| !head.work_tree.starts_with(tmp.path())));
    let missing = read_head(&tmp.join("missing/dir")).unwrap();
    assert_eq!(missing, ambient, "a missing start is not an error");
}

#[test]
fn broken_repositories_are_errors_not_panics() {
    let tmp = TempDir::new("git-broken").unwrap();
    let cases: [(&str, &str); 3] = [
        ("garbage-head", "not a head\n"),
        ("empty-head", ""),
        ("bad-ref", "ref: refs/heads/a b\n"),
    ];
    for (name, head) in cases {
        repo(&tmp.join(name), head);
        assert!(matches!(read_head(&tmp.join(name)), Err(GitError::Malformed(_))), "{name}");
    }
    let empty_gitdir = tmp.join("empty-gitdir");
    write(&empty_gitdir.join(".git"), "gitdir:   \n");
    assert!(matches!(read_head(&empty_gitdir), Err(GitError::Malformed(_))));
    let dangling = tmp.join("dangling");
    write(&dangling.join(".git"), "gitdir: ../nowhere/.git\n");
    assert!(matches!(read_head(&dangling), Err(GitError::Io { .. })));
    let huge = tmp.join("huge");
    let padding = " ".repeat(usize::try_from(MAX_GIT_FILE_BYTES).unwrap());
    repo(&huge, &format!("ref: refs/heads/main\n{padding}"));
    assert!(matches!(read_head(&huge), Err(GitError::Malformed(_))));
}

/// A FIFO named `HEAD` would block a plain `open` forever; it must be refused without opening it.
#[cfg(unix)]
#[cfg_attr(miri, ignore = "Miri cannot spawn the mkfifo process")]
#[test]
fn a_head_that_is_not_a_regular_file_is_refused_without_blocking() {
    let tmp = TempDir::new("git-fifo").unwrap();
    fs::create_dir_all(tmp.join(".git")).unwrap();
    let status = std::process::Command::new("mkfifo").arg(tmp.join(".git/HEAD")).status().unwrap();
    assert!(status.success());
    assert!(matches!(read_head(tmp.path()), Err(GitError::Malformed(_))));
}

#[test]
fn relative_start_is_refused() {
    assert!(matches!(read_head(Path::new("some/dir")), Err(GitError::RelativePath)));
}
