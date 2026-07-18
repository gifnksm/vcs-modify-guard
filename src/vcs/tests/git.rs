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
    testing::{AssertFileStatus, AssertRepositoryStatus, PathInTempDir},
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

const SUBDIR_CLEAN_FILE: &str = "subdir/clean_file.txt";
const SUBDIR_MODIFIED_FILE: &str = "subdir/modified_file.txt";
const SUBDIR_UNTRACKED_FILE: &str = "subdir/untracked_file.txt";

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
fn status_reports_nothing_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_modified_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_file: PathInTempDir,
) {
    let path = worktree_with_modified_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .modified([MODIFIED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_staged_file(backend: &dyn VcsBackend, worktree_with_staged_file: PathInTempDir) {
    let path = worktree_with_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .staged([STAGED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_modified_and_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_and_staged_file: PathInTempDir,
) {
    let path = worktree_with_modified_and_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .modified([MODIFIED_AND_STAGED_FILE])
        .staged([MODIFIED_AND_STAGED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_deleted_file: PathInTempDir,
) {
    let path = worktree_with_deleted_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .modified([DELETED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_index_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_index_deleted_file: PathInTempDir,
) {
    let path = worktree_with_index_deleted_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .staged([INDEX_DELETED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_untracked_file: PathInTempDir,
) {
    let path = worktree_with_untracked_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .untracked([UNTRACKED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_nothing_for_worktree_with_ignored_file(
    backend: &dyn VcsBackend,
    worktree_with_ignored_file: PathInTempDir,
) {
    let path = worktree_with_ignored_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_mixed_changes(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let path = worktree_with_mixed_changes.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .modified([MODIFIED_FILE, MODIFIED_AND_STAGED_FILE, DELETED_FILE])
        .staged([STAGED_FILE, MODIFIED_AND_STAGED_FILE, INDEX_DELETED_FILE])
        .untracked([UNTRACKED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_modified_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let path = worktree_with_modified_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .modified([SUBDIR_MODIFIED_FILE])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn status_reports_untracked_file_in_subdir_as_untracked_dir(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let path = worktree_with_untracked_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.status().unwrap();
    AssertRepositoryStatus::default()
        .untracked(["subdir/"])
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_nothing_for_clean_file(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(CLEAN_FILE)).unwrap();
    AssertFileStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_modified_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_file: PathInTempDir,
) {
    let path = worktree_with_modified_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(MODIFIED_FILE)).unwrap();
    AssertFileStatus::default().modified().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_staged_file: PathInTempDir,
) {
    let path = worktree_with_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(STAGED_FILE)).unwrap();
    AssertFileStatus::default().staged().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_modified_and_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_and_staged_file: PathInTempDir,
) {
    let path = worktree_with_modified_and_staged_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo
        .file_status(Path::new(MODIFIED_AND_STAGED_FILE))
        .unwrap();
    AssertFileStatus::default()
        .modified()
        .staged()
        .assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_untracked_file: PathInTempDir,
) {
    let path = worktree_with_untracked_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(UNTRACKED_FILE)).unwrap();
    AssertFileStatus::default().untracked().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_nothing_for_ignored_file(
    backend: &dyn VcsBackend,
    worktree_with_ignored_file: PathInTempDir,
) {
    let path = worktree_with_ignored_file.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(IGNORED_FILE)).unwrap();
    AssertFileStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_mixed_changes(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let path = worktree_with_mixed_changes.path();
    let repo = backend.open(path).unwrap().unwrap();

    let status = repo.file_status(Path::new(CLEAN_FILE)).unwrap();
    AssertFileStatus::default().assert(status);

    let status = repo.file_status(Path::new(MODIFIED_FILE)).unwrap();
    AssertFileStatus::default().modified().assert(status);

    let status = repo.file_status(Path::new(STAGED_FILE)).unwrap();
    AssertFileStatus::default().staged().assert(status);

    let status = repo
        .file_status(Path::new(MODIFIED_AND_STAGED_FILE))
        .unwrap();
    AssertFileStatus::default()
        .modified()
        .staged()
        .assert(status);

    let status = repo.file_status(Path::new(UNTRACKED_FILE)).unwrap();
    AssertFileStatus::default().untracked().assert(status);

    let status = repo.file_status(Path::new(IGNORED_FILE)).unwrap();
    AssertFileStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_rejects_non_existent_file(backend: &dyn VcsBackend, clean_worktree: PathInTempDir) {
    let path = clean_worktree.path();
    let repo = backend.open(path).unwrap().unwrap();
    let err = repo
        .file_status(Path::new("non_existent_file.txt"))
        .unwrap_err();
    assert_matches!(err, VcsStatusError::PathNotFound { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_status_normalizes_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();

    let path = PathBuf::from(format!("subdir//{CLEAN_FILE}"));
    let status = repo.file_status(&path).unwrap();
    AssertFileStatus::default().assert(status);

    let path = PathBuf::from(format!("subdir/./{CLEAN_FILE}"));
    let status = repo.file_status(&path).unwrap();
    AssertFileStatus::default().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_rejects_path_contains_dotdot(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let dir_name = path.file_name().unwrap().to_str().unwrap();
    let repo = backend.open(path).unwrap().unwrap();

    let path = PathBuf::from(format!("../{dir_name}/{SUBDIR_CLEAN_FILE}"));
    let err = repo.file_status(&path).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });

    let path = PathBuf::from(format!("subdir/../{SUBDIR_CLEAN_FILE}"));
    let err = repo.file_status(&path).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });

    let path = PathBuf::from(format!("./{SUBDIR_CLEAN_FILE}"));
    let err = repo.file_status(&path).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_status_rejects_empty_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();

    let err = repo.file_status(Path::new("")).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_status_rejects_absolute_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();

    let absolute_path = PathBuf::from(format!("/{SUBDIR_CLEAN_FILE}"));
    let err = repo.file_status(&absolute_path).unwrap_err();
    assert_matches!(err, VcsStatusError::InvalidWorktreeRelativePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_modified_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let path = worktree_with_modified_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(SUBDIR_MODIFIED_FILE)).unwrap();
    AssertFileStatus::default().modified().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_reports_untracked_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let path = worktree_with_untracked_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let status = repo.file_status(Path::new(SUBDIR_UNTRACKED_FILE)).unwrap();
    AssertFileStatus::default().untracked().assert(status);
}

#[apply(all_backends)]
#[rstest]
fn file_status_rejects_directory_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let path = clean_worktree_with_subdir.path();
    let repo = backend.open(path).unwrap().unwrap();
    let err = repo.file_status(Path::new("subdir")).unwrap_err();
    assert_matches!(err, VcsStatusError::Backend { .. });
}
