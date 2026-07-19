use std::{
    assert_matches,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use assert_fs::prelude::*;
use rstest::*;
use rstest_reuse::*;

use crate::{
    VcsStatusError,
    repository::FileChange,
    testing::{AssertFileChange, AssertRepositoryChanges, PathInTempDir},
    vcs::{self, VcsBackend},
};

#[must_use]
fn git_command<P>(current_dir: P) -> assert_cmd::Command
where
    P: AsRef<Path>,
{
    let mut cmd = assert_cmd::Command::new("git");
    cmd.current_dir(current_dir).envs([
        ("GIT_AUTHOR_NAME", "Test User"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "Test User"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
    ]);
    cmd
}

#[must_use]
fn git_init<P>(current_dir: P) -> assert_cmd::assert::Assert
where
    P: AsRef<Path>,
{
    git_command(current_dir).args(["init"]).assert()
}

#[must_use]
fn git_init_bare<P>(current_dir: P) -> assert_cmd::assert::Assert
where
    P: AsRef<Path>,
{
    git_command(current_dir).args(["init", "--bare"]).assert()
}

#[must_use]
fn git_add<P, I, S>(current_dir: P, pathspec: I) -> assert_cmd::assert::Assert
where
    P: AsRef<Path>,
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git_command(current_dir).arg("add").args(pathspec).assert()
}

#[must_use]
fn git_rm<P, I, S>(current_dir: P, pathspec: I) -> assert_cmd::assert::Assert
where
    P: AsRef<Path>,
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git_command(current_dir).arg("rm").args(pathspec).assert()
}

#[must_use]
fn git_commit<P>(current_dir: P) -> assert_cmd::assert::Assert
where
    P: AsRef<Path>,
{
    git_command(current_dir)
        .args(["commit", "-m", "commit", "--allow-empty"])
        .assert()
}

#[fixture]
fn non_git_directory() -> PathInTempDir {
    PathInTempDir::new()
}

const CLEAN_FILE: &str = "clean_file.txt";
const MODIFIED_FILE: &str = "modified_file.txt";
const STAGED_FILE: &str = "staged_file.txt";
const MODIFIED_AND_STAGED_FILE: &str = "modified_and_staged_file.txt";
const DELETED_FILE: &str = "deleted_file.txt";
const INDEX_DELETED_FILE: &str = "index_deleted_file.txt";
const UNTRACKED_FILE: &str = "untracked_file.txt";
const IGNORED_FILE: &str = "ignored_file.txt";
#[cfg(unix)]
const SYMLINK_FILE: &str = "symlink_file.txt";

#[fixture]
fn clean_worktree() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(CLEAN_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path
}

#[fixture]
fn worktree_with_modified_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(MODIFIED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path
}

#[fixture]
fn worktree_with_staged_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(STAGED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(STAGED_FILE).write_str("Staged content").unwrap();
    git_add(&path, ["."]).success();
    path
}

#[fixture]
fn worktree_with_modified_and_staged_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(MODIFIED_AND_STAGED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(MODIFIED_AND_STAGED_FILE)
        .write_str("Staged content")
        .unwrap();
    git_add(&path, ["."]).success();
    path.child(MODIFIED_AND_STAGED_FILE)
        .write_str("Modified content")
        .unwrap();
    path
}

#[fixture]
fn worktree_with_deleted_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(DELETED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    fs::remove_file(path.child(DELETED_FILE)).unwrap();
    path
}

#[fixture]
fn worktree_with_index_deleted_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(INDEX_DELETED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    git_rm(&path, [INDEX_DELETED_FILE]).success();
    path
}

#[fixture]
fn worktree_with_untracked_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(UNTRACKED_FILE).touch().unwrap();
    path
}

#[fixture]
fn worktree_with_ignored_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(".gitignore").write_str(IGNORED_FILE).unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(IGNORED_FILE).touch().unwrap();
    path
}

#[fixture]
fn worktree_with_mixed_changes() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(".gitignore").write_str(IGNORED_FILE).unwrap();
    path.child(CLEAN_FILE).touch().unwrap();
    path.child(MODIFIED_FILE).touch().unwrap();
    path.child(STAGED_FILE).touch().unwrap();
    path.child(MODIFIED_AND_STAGED_FILE).touch().unwrap();
    path.child(DELETED_FILE).touch().unwrap();
    path.child(INDEX_DELETED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(STAGED_FILE).write_str("Staged content").unwrap();
    path.child(MODIFIED_AND_STAGED_FILE)
        .write_str("Staged content")
        .unwrap();
    git_add(&path, ["."]).success();
    path.child(MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path.child(MODIFIED_AND_STAGED_FILE)
        .write_str("Modified content")
        .unwrap();
    fs::remove_file(path.child(DELETED_FILE)).unwrap();
    git_rm(&path, [INDEX_DELETED_FILE]).success();
    path.child(UNTRACKED_FILE).touch().unwrap();
    path.child(IGNORED_FILE).touch().unwrap();
    path
}

#[cfg(unix)]
#[fixture]
fn worktree_with_symlink() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(CLEAN_FILE).touch().unwrap();
    std::os::unix::fs::symlink(CLEAN_FILE, path.path().join(SYMLINK_FILE)).unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(CLEAN_FILE)
        .write_str("Modified content")
        .unwrap();
    path
}

const SUBDIR_CLEAN_FILE: &str = "subdir/clean_file.txt";
const SUBDIR_MODIFIED_FILE: &str = "subdir/modified_file.txt";
const SUBDIR_UNTRACKED_FILE: &str = "subdir/untracked_file.txt";
const SUBDIR_IGNORED_FILE: &str = "subdir/ignored_file.txt";

#[fixture]
fn clean_worktree_with_subdir() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(SUBDIR_CLEAN_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path
}

#[fixture]
fn worktree_with_modified_subdir() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(SUBDIR_MODIFIED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(SUBDIR_MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path
}

#[fixture]
fn worktree_with_untracked_subdir() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(SUBDIR_UNTRACKED_FILE).touch().unwrap();
    path
}

#[fixture]
fn worktree_with_ignored_subdir() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(".gitignore").write_str("subdir/").unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(SUBDIR_IGNORED_FILE).touch().unwrap();
    path
}

#[fixture]
fn bare_repository() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init_bare(&path).success();
    path
}

#[fixture]
fn non_existent_path() -> PathInTempDir {
    let mut path = PathInTempDir::new();

    let non_existent_path = path.child("non_existent_path");
    path.set_path(non_existent_path.path());

    path
}

#[cfg(unix)]
#[fixture]
fn inaccessible_path() -> PathInTempDir {
    use std::os::unix::fs::PermissionsExt as _;

    let mut path = PathInTempDir::new();

    let parent = path.child("parent");
    let inaccessible = parent.child("inaccessible");
    path.set_path(inaccessible.path());
    fs::create_dir(&parent).unwrap();

    let perms = fs::metadata(&parent).unwrap().permissions();
    let mut inaccessible_perms = perms.clone();
    inaccessible_perms.set_mode(0o000);
    fs::set_permissions(&parent, inaccessible_perms).unwrap();

    path.set_drop_guard(move |_path| {
        fs::set_permissions(&parent, perms).unwrap();
    });

    path
}

#[template]
#[rstest]
#[cfg_attr(feature = "git-libgit2", case::libgit2(&vcs::git_libgit2::BACKEND))]
fn all_backends(#[case] backend: &dyn VcsBackend) {}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.discover(path).unwrap().unwrap();
    assert_eq!(repo.worktree(), path);
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_for_clean_worktree_subdir(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.child("subdir");
    let repo = backend.discover(&path).unwrap().unwrap();
    assert_eq!(repo.worktree(), clean_worktree_with_subdir.path());
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_for_worktree_file(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.child(SUBDIR_CLEAN_FILE);
    let repo = backend.discover(&path).unwrap().unwrap();
    assert_eq!(repo.worktree(), clean_worktree_with_subdir.path());
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_none_for_non_git_directory(
    backend: &dyn VcsBackend,
    non_git_directory: PathInTempDir,
) {
    let path = non_git_directory.path();
    assert!(backend.discover(path).unwrap().is_none());
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_err_for_bare_repository(
    backend: &dyn VcsBackend,
    bare_repository: PathInTempDir,
) {
    let path = bare_repository.path();
    let err = backend.discover(path).unwrap_err();
    assert_matches!(err, VcsStatusError::RepositoryWithoutWorktree { .. });
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_err_for_non_existent_path(
    backend: &dyn VcsBackend,
    non_existent_path: PathInTempDir,
) {
    let path = non_existent_path.path();
    let err = backend.discover(path).unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotFound { .. });
}

#[cfg(unix)]
#[apply(all_backends)]
#[rstest]
fn discover_returns_err_for_inaccessible_path(
    backend: &dyn VcsBackend,
    inaccessible_path: PathInTempDir,
) {
    let path = inaccessible_path.path();
    let err = backend.discover(path).unwrap_err();
    assert_matches!(err, VcsStatusError::InaccessiblePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn open_returns_repository_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    assert_eq!(repo.worktree(), path);
}

#[apply(all_backends)]
#[rstest]
fn open_returns_none_for_worktree_subdir(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.child("subdir");
    assert!(backend.open(&path).unwrap().is_none());
}

#[apply(all_backends)]
#[rstest]
fn open_returns_err_for_worktree_file(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.child(SUBDIR_CLEAN_FILE);
    let err = backend.open(&path).unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotADirectory { .. });
}

#[apply(all_backends)]
#[rstest]
fn open_returns_none_for_non_git_directory(
    backend: &dyn VcsBackend,
    non_git_directory: PathInTempDir,
) {
    let path = non_git_directory.path();
    assert!(backend.open(path).unwrap().is_none());
}

#[apply(all_backends)]
#[rstest]
fn open_returns_err_for_bare_repository(backend: &dyn VcsBackend, bare_repository: PathInTempDir) {
    let path = bare_repository.path();
    let err = backend.open(path).unwrap_err();
    assert_matches!(err, VcsStatusError::RepositoryWithoutWorktree { .. });
}

#[apply(all_backends)]
#[rstest]
fn open_returns_err_for_non_existent_path(
    backend: &dyn VcsBackend,
    non_existent_path: PathInTempDir,
) {
    let path = non_existent_path.path();
    let err = backend.open(path).unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotFound { .. });
}

#[cfg(unix)]
#[apply(all_backends)]
#[rstest]
fn open_returns_err_for_inaccessible_path(
    backend: &dyn VcsBackend,
    inaccessible_path: PathInTempDir,
) {
    let path = inaccessible_path.path();
    let err = backend.open(path).unwrap_err();
    assert_matches!(err, VcsStatusError::InaccessiblePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_returns_none_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap();
    assert!(changes.is_none());
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_modified_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_file: PathInTempDir,
) {
    let path = worktree_with_modified_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .modified([MODIFIED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_staged_file: PathInTempDir,
) {
    let path = worktree_with_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .staged([STAGED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_modified_and_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_and_staged_file: PathInTempDir,
) {
    let path = worktree_with_modified_and_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .modified([MODIFIED_AND_STAGED_FILE])
        .staged([MODIFIED_AND_STAGED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_deleted_file: PathInTempDir,
) {
    let path = worktree_with_deleted_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .modified([DELETED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_index_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_index_deleted_file: PathInTempDir,
) {
    let path = worktree_with_index_deleted_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .staged([INDEX_DELETED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_untracked_file: PathInTempDir,
) {
    let path = worktree_with_untracked_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .untracked([UNTRACKED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_returns_none_for_worktree_with_ignored_file(
    backend: &dyn VcsBackend,
    worktree_with_ignored_file: PathInTempDir,
) {
    let path = worktree_with_ignored_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap();
    assert!(changes.is_none());
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_mixed_changes(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let path = worktree_with_mixed_changes.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .modified([MODIFIED_FILE, MODIFIED_AND_STAGED_FILE, DELETED_FILE])
        .staged([STAGED_FILE, MODIFIED_AND_STAGED_FILE, INDEX_DELETED_FILE])
        .untracked([UNTRACKED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_modified_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let path = worktree_with_modified_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .modified([SUBDIR_MODIFIED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_reports_untracked_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let path = worktree_with_untracked_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let changes = repo.repository_changes().unwrap().unwrap();
    AssertRepositoryChanges::default()
        .untracked([SUBDIR_UNTRACKED_FILE])
        .assert(changes);
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_none_for_clean_file(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(CLEAN_FILE)).unwrap();
    assert!(change.is_none());
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_modified_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_file: PathInTempDir,
) {
    let path = worktree_with_modified_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(MODIFIED_FILE)).unwrap().unwrap();
    AssertFileChange::new(MODIFIED_FILE)
        .modified()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_staged_file: PathInTempDir,
) {
    let path = worktree_with_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(STAGED_FILE)).unwrap().unwrap();
    AssertFileChange::new(STAGED_FILE).staged().assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_modified_and_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_and_staged_file: PathInTempDir,
) {
    let path = worktree_with_modified_and_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo
        .file_change(Path::new(MODIFIED_AND_STAGED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(MODIFIED_AND_STAGED_FILE)
        .modified()
        .staged()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_untracked_file: PathInTempDir,
) {
    let path = worktree_with_untracked_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo
        .file_change(Path::new(UNTRACKED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(UNTRACKED_FILE)
        .untracked()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_none_for_ignored_file(
    backend: &dyn VcsBackend,
    worktree_with_ignored_file: PathInTempDir,
) {
    let path = worktree_with_ignored_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(IGNORED_FILE)).unwrap();
    assert!(change.is_none());
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_mixed_changes(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let path = worktree_with_mixed_changes.path();
    let repo = backend.open(path).unwrap().unwrap();

    let change = repo.file_change(Path::new(CLEAN_FILE)).unwrap();
    assert!(change.is_none());

    let change = repo.file_change(Path::new(MODIFIED_FILE)).unwrap().unwrap();
    AssertFileChange::new(MODIFIED_FILE)
        .modified()
        .assert(change);

    let change = repo.file_change(Path::new(STAGED_FILE)).unwrap().unwrap();
    AssertFileChange::new(STAGED_FILE).staged().assert(change);

    let change = repo
        .file_change(Path::new(MODIFIED_AND_STAGED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(MODIFIED_AND_STAGED_FILE)
        .modified()
        .staged()
        .assert(change);

    let change = repo
        .file_change(Path::new(UNTRACKED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(UNTRACKED_FILE)
        .untracked()
        .assert(change);

    let change = repo.file_change(Path::new(IGNORED_FILE)).unwrap();
    assert!(change.is_none());
}

#[cfg(unix)]
#[apply(all_backends)]
#[rstest]
fn file_change_resolves_symlink(backend: &dyn VcsBackend, worktree_with_symlink: PathInTempDir) {
    let path = worktree_with_symlink.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(SYMLINK_FILE)).unwrap().unwrap();
    AssertFileChange::new(CLEAN_FILE).modified().assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_non_existent_file(backend: &dyn VcsBackend, clean_worktree: PathInTempDir) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let err = repo
        .file_change(Path::new("non_existent_file.txt"))
        .unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotFound { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_canonicalized_path(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let repo_path = worktree_with_modified_subdir.path();
    let dir_name = repo_path.file_name().unwrap().to_str().unwrap();
    let repo = backend.open(repo_path).unwrap().unwrap();

    let path = PathBuf::from(format!("subdir//{MODIFIED_FILE}"));
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);

    let path = PathBuf::from(format!("./{SUBDIR_MODIFIED_FILE}"));
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);

    let path = PathBuf::from(format!("subdir/./{MODIFIED_FILE}"));
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);

    let path = PathBuf::from(format!("../{dir_name}/{SUBDIR_MODIFIED_FILE}"));
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);

    let path = PathBuf::from(format!("subdir/../{SUBDIR_MODIFIED_FILE}"));
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);

    let path = repo_path.join(SUBDIR_MODIFIED_FILE);
    let change = repo.file_change(&path).unwrap().unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_empty_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();

    let err = repo.file_change(Path::new("")).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_modified_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let path = worktree_with_modified_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo
        .file_change(Path::new(SUBDIR_MODIFIED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .modified()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_reports_untracked_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let path = worktree_with_untracked_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo
        .file_change(Path::new(SUBDIR_UNTRACKED_FILE))
        .unwrap()
        .unwrap();
    AssertFileChange::new(SUBDIR_UNTRACKED_FILE)
        .untracked()
        .assert(change);
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_directory_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let err = repo.file_change(Path::new("subdir")).unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotAFile { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_none_for_file_in_ignored_directory_path(
    backend: &dyn VcsBackend,
    worktree_with_ignored_subdir: PathInTempDir,
) {
    let path = worktree_with_ignored_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let change = repo.file_change(Path::new(SUBDIR_IGNORED_FILE)).unwrap();
    assert!(change.is_none());
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_and_file_change_agree_for_paths_reported_individually(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let path = worktree_with_mixed_changes.path();
    let repo = backend.open(path).unwrap().unwrap();
    let repo_changes = repo.repository_changes().unwrap().unwrap();

    let paths = [
        (CLEAN_FILE, None),
        (MODIFIED_FILE, None),
        (STAGED_FILE, None),
        (MODIFIED_AND_STAGED_FILE, None),
        (DELETED_FILE, Some((true, false))),
        (INDEX_DELETED_FILE, Some((false, true))),
        (UNTRACKED_FILE, None),
        (IGNORED_FILE, None),
    ];

    let mut modified_count = 0;
    let mut staged_count = 0;
    let mut untracked_count = 0;

    let repo_modified_paths = repo_changes
        .modified_files()
        .map(FileChange::path)
        .collect::<Vec<_>>();
    let repo_staged_paths = repo_changes
        .staged_files()
        .map(FileChange::path)
        .collect::<Vec<_>>();
    let repo_untracked_paths = repo_changes
        .untracked_files()
        .map(FileChange::path)
        .collect::<Vec<_>>();

    for (path, deleted) in &paths {
        let path = Path::new(path);
        if let Some((wt_deleted, index_deleted)) = *deleted {
            if wt_deleted {
                modified_count += 1;
            }
            if index_deleted {
                staged_count += 1;
            }
            continue;
        }
        let Some(file_change) = repo.file_change(path).unwrap() else {
            continue;
        };
        let mut expected = AssertFileChange::new(path);
        if repo_modified_paths.contains(&path) {
            modified_count += 1;
            expected = expected.modified();
        }
        if repo_staged_paths.contains(&path) {
            staged_count += 1;
            expected = expected.staged();
        }
        if repo_untracked_paths.contains(&path) {
            untracked_count += 1;
            expected = expected.untracked();
        }
        expected.assert(file_change);
    }

    assert_eq!(repo_modified_paths.len(), modified_count);
    assert_eq!(repo_staged_paths.len(), staged_count);
    assert_eq!(repo_untracked_paths.len(), untracked_count);
}
