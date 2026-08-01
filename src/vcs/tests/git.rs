use std::{
    assert_matches,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};
#[cfg(all(unix, not(target_vendor = "apple")))]
use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};

use assert_fs::prelude::*;
use rstest::*;
use rstest_reuse::*;

use crate::{
    ModifyGuardError::{self, *},
    VcsRepository,
    repository::{FileChange, RepositoryChanges},
    testing::{AssertFileChange, AssertRepositoryChanges, PathInTempDir},
    vcs::{self, VcsBackend},
};

#[must_use]
fn git_command<P>(current_dir: P) -> assert_cmd::Command
where
    P: AsRef<Path>,
{
    let mut cmd = assert_cmd::Command::new("git");
    cmd.current_dir(current_dir)
        .envs([
            ("GIT_AUTHOR_NAME", "Test User"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "Test User"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR");
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

fn git_current_branch<P>(current_dir: P) -> String
where
    P: AsRef<Path>,
{
    let output = git_command(current_dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end_matches('\n')
        .to_owned()
}

fn git_absolute_git_dir<P>(current_dir: P) -> PathBuf
where
    P: AsRef<Path>,
{
    let output = git_command(current_dir)
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    PathBuf::from(stdout.trim_end_matches('\n'))
}

// macOS filesystems reject arbitrary non-UTF-8 byte sequences with EILSEQ,
// so these fixtures are limited to Unix platforms that accept raw byte paths.
#[cfg(all(unix, not(target_vendor = "apple")))]
fn non_utf8_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}

#[cfg(all(unix, not(target_vendor = "apple")))]
fn non_utf8_repo_dir() -> PathBuf {
    non_utf8_path(b"repo-\xFF")
}

#[cfg(all(unix, not(target_vendor = "apple")))]
fn non_utf8_untracked_file() -> PathBuf {
    non_utf8_path(b"non-utf8-\xFF.txt")
}

#[fixture]
fn non_git_directory() -> PathInTempDir {
    PathInTempDir::new()
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[fixture]
fn clean_worktree_in_non_utf8_directory() -> PathInTempDir {
    let mut path = PathInTempDir::new();
    let repo_path = path.path().join(non_utf8_repo_dir());
    fs::create_dir(&repo_path).unwrap();
    git_init(&repo_path).success();
    path.set_path(repo_path);
    path.child(CLEAN_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path
}

struct LinkedWorktreePaths {
    _root: PathInTempDir,
    linked_worktree: PathBuf,
    linked_git_dir: PathBuf,
}

impl LinkedWorktreePaths {
    fn linked_worktree(&self) -> &Path {
        &self.linked_worktree
    }

    fn linked_git_dir(&self) -> &Path {
        &self.linked_git_dir
    }
}

#[fixture]
fn linked_worktree_paths() -> LinkedWorktreePaths {
    let root = PathInTempDir::new();
    root.child("main").create_dir_all().unwrap();
    let main_worktree = root.path().join("main");
    let linked_worktree = root.path().join("linked");

    git_init(&main_worktree).success();
    root.child("main/clean_file.txt").touch().unwrap();
    git_add(&main_worktree, ["."]).success();
    git_commit(&main_worktree).success();
    git_command(&main_worktree)
        .arg("worktree")
        .arg("add")
        .arg(linked_worktree.as_os_str())
        .assert()
        .success();
    let linked_git_dir = git_absolute_git_dir(&linked_worktree);

    LinkedWorktreePaths {
        _root: root,
        linked_worktree,
        linked_git_dir,
    }
}

const CLEAN_FILE: &str = "clean_file.txt";
const MODIFIED_FILE: &str = "modified_file.txt";
const STAGED_FILE: &str = "staged_file.txt";
const MODIFIED_AND_STAGED_FILE: &str = "modified_and_staged_file.txt";
const DELETED_FILE: &str = "deleted_file.txt";
const DELETED_DIR_FILE: &str = "deleted_dir/deleted_file.txt";
const DELETED_DIR_FILE2: &str = "deleted_dir/another_deleted_file.txt";
const INDEX_DELETED_FILE: &str = "index_deleted_file.txt";
const CONFLICTED_FILE: &str = "conflicted_file.txt";
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
fn worktree_with_deleted_directory() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(DELETED_DIR_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    fs::remove_file(path.child(DELETED_DIR_FILE)).unwrap();
    fs::remove_dir(path.child("deleted_dir")).unwrap();
    path
}

#[fixture]
fn worktree_with_deleted_directory_multiple_files() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(DELETED_DIR_FILE).touch().unwrap();
    path.child(DELETED_DIR_FILE2).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    fs::remove_file(path.child(DELETED_DIR_FILE)).unwrap();
    fs::remove_file(path.child(DELETED_DIR_FILE2)).unwrap();
    fs::remove_dir(path.child("deleted_dir")).unwrap();
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
fn worktree_with_conflicted_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(CONFLICTED_FILE).write_str("base\n").unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();

    let current_branch = git_current_branch(&path);
    git_command(&path)
        .args(["branch", "other"])
        .assert()
        .success();

    path.child(CONFLICTED_FILE).write_str("ours\n").unwrap();
    git_add(&path, [CONFLICTED_FILE]).success();
    git_commit(&path).success();

    git_command(&path)
        .args(["checkout", "other"])
        .assert()
        .success();
    path.child(CONFLICTED_FILE).write_str("theirs\n").unwrap();
    git_add(&path, [CONFLICTED_FILE]).success();
    git_commit(&path).success();

    git_command(&path)
        .args(["merge", current_branch.as_str()])
        .assert()
        .failure();

    path
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[fixture]
fn worktree_with_non_utf8_untracked_file() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(non_utf8_untracked_file()).touch().unwrap();
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
const SUBDIR1_MODIFIED_FILE: &str = "subdir1/modified_file.txt";
const SUBDIR1_UNTRACKED_FILE: &str = "subdir1/untracked_file.txt";
const LITERAL_SUBDIR_MODIFIED_FILE: &str = "subdir[1]/modified_file.txt";
const GLOB_MATCHING_SUBDIR_MODIFIED_FILE: &str = "subdir1/modified_file.txt";

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
fn worktree_with_root_and_subdir_changes() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(MODIFIED_FILE).touch().unwrap();
    path.child(SUBDIR_MODIFIED_FILE).touch().unwrap();
    path.child(SUBDIR1_MODIFIED_FILE).touch().unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path.child(SUBDIR_MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path.child(SUBDIR1_MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path.child(UNTRACKED_FILE).touch().unwrap();
    path.child(SUBDIR_UNTRACKED_FILE).touch().unwrap();
    path.child(SUBDIR1_UNTRACKED_FILE).touch().unwrap();
    path
}

#[fixture]
fn worktree_with_literal_and_glob_matching_subdirs() -> PathInTempDir {
    let path = PathInTempDir::new();
    git_init(&path).success();
    path.child(LITERAL_SUBDIR_MODIFIED_FILE).touch().unwrap();
    path.child(GLOB_MATCHING_SUBDIR_MODIFIED_FILE)
        .touch()
        .unwrap();
    git_add(&path, ["."]).success();
    git_commit(&path).success();
    path.child(LITERAL_SUBDIR_MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
    path.child(GLOB_MATCHING_SUBDIR_MODIFIED_FILE)
        .write_str("Modified content")
        .unwrap();
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
#[cfg_attr(feature = "git-gix", case::gix(&vcs::git_gix::BACKEND))]
#[cfg_attr(feature = "git-libgit2", case::libgit2(&vcs::git_libgit2::BACKEND))]
#[cfg_attr(feature = "git-cli", case::cli(&vcs::git_cli::BACKEND))]
fn all_backends(#[case] backend: &dyn VcsBackend) {}

#[track_caller]
fn assert_discover_ok<P>(backend: &dyn VcsBackend, query: P, expected_worktree: Option<&Path>)
where
    P: AsRef<Path>,
{
    let query = query.as_ref();
    let repo = backend.discover(query).unwrap();
    assert_eq!(repo.as_ref().map(|repo| repo.worktree()), expected_worktree);
}

#[track_caller]
#[must_use]
fn assert_discover_err<P>(backend: &dyn VcsBackend, query: P) -> ModifyGuardError
where
    P: AsRef<Path>,
{
    let query = query.as_ref();
    backend.discover(query).unwrap_err()
}

macro_rules! assert_discover_err_matches {
    ($backend:expr, $query:expr, $pattern:pat_param $(if $guard: expr)? $(,)?) => {{
        let err = assert_discover_err($backend, $query);
        assert_matches!(err, $pattern $(if $guard)?);
    }};
}

#[track_caller]
fn assert_open_ok<P>(
    backend: &dyn VcsBackend,
    query: P,
    expected_worktree: Option<&Path>,
) -> Option<Box<dyn VcsRepository>>
where
    P: AsRef<Path>,
{
    let query = query.as_ref();
    let repo = backend.open(query).unwrap();
    assert_eq!(repo.as_ref().map(|repo| repo.worktree()), expected_worktree);
    repo
}

#[track_caller]
#[must_use]
fn assert_open_err<P>(backend: &dyn VcsBackend, query: P) -> ModifyGuardError
where
    P: AsRef<Path>,
{
    let query = query.as_ref();
    backend.open(query).unwrap_err()
}

macro_rules! assert_open_err_matches {
    ($backend:expr, $query:expr, $pattern:pat_param $(if $guard: expr)? $(,)?) => {{
        let err = assert_open_err($backend, $query);
        assert_matches!(err, $pattern $(if $guard)?);
    }};
}

#[track_caller]
#[must_use]
fn open_repo<P>(backend: &dyn VcsBackend, query: P) -> Box<dyn VcsRepository>
where
    P: AsRef<Path>,
{
    let query = query.as_ref();
    backend.open(query).unwrap().unwrap()
}

#[track_caller]
fn assert_repository_changes_ok<R>(repo: R) -> Option<RepositoryChanges>
where
    R: AsRef<dyn VcsRepository>,
{
    repo.as_ref().repository_changes().unwrap()
}

#[track_caller]
fn assert_path_changes_ok<R, P>(repo: R, wt_path: P) -> Option<RepositoryChanges>
where
    R: AsRef<dyn VcsRepository>,
    P: AsRef<Path>,
{
    repo.as_ref().path_changes(wt_path.as_ref()).unwrap()
}

#[track_caller]
fn assert_path_changes_err<R, P>(repo: R, wt_path: P) -> ModifyGuardError
where
    R: AsRef<dyn VcsRepository>,
    P: AsRef<Path>,
{
    repo.as_ref().path_changes(wt_path.as_ref()).unwrap_err()
}

macro_rules! assert_path_changes_err_matches {
    ($repo:expr, $wt_path:expr, $pattern:pat_param $(if $guard: expr)? $(,)?) => {{
        let err = assert_path_changes_err($repo, $wt_path);
        assert_matches!(err, $pattern $(if $guard)?);
    }};
}

#[track_caller]
fn assert_file_change_ok<R, P>(repo: R, wt_path: P) -> Option<FileChange>
where
    R: AsRef<dyn VcsRepository>,
    P: AsRef<Path>,
{
    repo.as_ref().file_change(wt_path.as_ref()).unwrap()
}

#[track_caller]
fn assert_file_change_err<R, P>(repo: R, wt_path: P) -> ModifyGuardError
where
    R: AsRef<dyn VcsRepository>,
    P: AsRef<Path>,
{
    repo.as_ref().file_change(wt_path.as_ref()).unwrap_err()
}

macro_rules! assert_file_change_err_matches {
    ($repo:expr, $wt_path:expr, $pattern:pat_param $(if $guard: expr)? $(,)?) => {{
        let err = assert_file_change_err($repo, $wt_path);
        assert_matches!(err, $pattern $(if $guard)?);
    }};
}

impl AssertRepositoryChanges {
    #[track_caller]
    fn assert_repository_changes<R>(&self, repo: R) -> &Self
    where
        R: AsRef<dyn VcsRepository>,
    {
        let changes = assert_repository_changes_ok(repo);
        self.assert(changes)
    }

    #[track_caller]
    fn assert_path_changes<R, P>(&self, repo: R, wt_path: P) -> &Self
    where
        R: AsRef<dyn VcsRepository>,
        P: AsRef<Path>,
    {
        let changes = assert_path_changes_ok(repo, wt_path);
        self.assert(changes)
    }

    #[track_caller]
    fn assert_file_change<R, P>(&self, repo: R, wt_path: P) -> &Self
    where
        R: AsRef<dyn VcsRepository>,
        P: AsRef<Path>,
    {
        let wt_path = wt_path.as_ref();
        let change = assert_file_change_ok(repo, wt_path);
        self.file_change(wt_path).assert(change);
        self
    }
}

impl AssertFileChange {
    #[track_caller]
    fn assert_file_change<R, P>(&self, repo: R, wt_path: P) -> &Self
    where
        R: AsRef<dyn VcsRepository>,
        P: AsRef<Path>,
    {
        let change = assert_file_change_ok(repo, wt_path);
        self.assert(change)
    }
}

#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_repository_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let worktree_path = clean_worktree.path();
    let queries = [worktree_path, &worktree_path.join(".git")];
    for query in queries {
        assert_discover_ok(backend, query, Some(worktree_path));
        assert_open_ok(backend, query, Some(worktree_path));
    }
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_and_open_returns_none_for_worktree_subdir(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let worktree_path = clean_worktree_with_subdir.path();
    let queries = [
        &worktree_path.join("subdir"),
        &worktree_path.join(".git/objects"),
    ];
    for query in queries {
        assert_discover_ok(backend, query, Some(worktree_path));
        assert_open_ok(backend, query, None);
    }
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_and_open_returns_err_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let worktree_path = clean_worktree_with_subdir.path();
    let query = &worktree_path.join(SUBDIR_CLEAN_FILE);
    assert_discover_ok(backend, query, Some(worktree_path));
    assert_open_err_matches!(backend, query, PathNotADirectory { .. });
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_repository_for_worktree_in_non_utf8_directory(
    backend: &dyn VcsBackend,
    clean_worktree_in_non_utf8_directory: PathInTempDir,
) {
    let worktree_path = clean_worktree_in_non_utf8_directory.path();
    assert_discover_ok(backend, worktree_path, Some(worktree_path));
    assert_open_ok(backend, worktree_path, Some(worktree_path));
}

#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_repository_for_linked_worktree_git_dir(
    backend: &dyn VcsBackend,
    linked_worktree_paths: LinkedWorktreePaths,
) {
    let worktree_path = linked_worktree_paths.linked_worktree();
    let query = linked_worktree_paths.linked_git_dir();
    assert_discover_ok(backend, query, Some(worktree_path));
    assert_open_ok(backend, query, Some(worktree_path));
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_repository_and_open_returns_none_for_linked_worktree_git_dir(
    backend: &dyn VcsBackend,
    linked_worktree_paths: LinkedWorktreePaths,
) {
    let worktree_path = linked_worktree_paths.linked_worktree();
    let query = &linked_worktree_paths.linked_git_dir().join("logs");
    assert_discover_ok(backend, query, Some(worktree_path));
    assert_open_ok(backend, query, None);
}

#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_none_for_non_git_directory(
    backend: &dyn VcsBackend,
    non_git_directory: PathInTempDir,
) {
    let query = non_git_directory.path();
    assert_discover_ok(backend, query, None);
    assert_open_ok(backend, query, None);
}

#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_err_for_bare_repository(
    backend: &dyn VcsBackend,
    bare_repository: PathInTempDir,
) {
    let query = bare_repository.path();
    assert_discover_err_matches!(backend, query, RepositoryWithoutWorktree { .. });
    assert_open_err_matches!(backend, query, RepositoryWithoutWorktree { .. });
}

#[apply(all_backends)]
#[rstest]
fn discover_returns_err_and_open_returns_none_for_bare_repository_subdir(
    backend: &dyn VcsBackend,
    bare_repository: PathInTempDir,
) {
    let repo_path = bare_repository.path();
    let query = &repo_path.join("objects");
    assert_discover_err_matches!(backend, query, RepositoryWithoutWorktree { .. });
    assert_open_ok(backend, query, None);
}

#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_err_for_non_existent_path(
    backend: &dyn VcsBackend,
    non_existent_path: PathInTempDir,
) {
    let query = non_existent_path.path();
    assert_discover_err_matches!(backend, query, PathNotFound { .. });
    assert_open_err_matches!(backend, query, PathNotFound { .. });
}

#[cfg(unix)]
#[apply(all_backends)]
#[rstest]
fn discover_and_open_returns_err_for_inaccessible_path(
    backend: &dyn VcsBackend,
    inaccessible_path: PathInTempDir,
) {
    let query = inaccessible_path.path();
    assert_discover_err_matches!(backend, query, InaccessiblePath { .. });
    assert_open_err_matches!(backend, query, InaccessiblePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn query_changes_returns_none_for_clean_worktree(
    backend: &dyn VcsBackend,
    clean_worktree: PathInTempDir,
) {
    let worktree_path = clean_worktree.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, CLEAN_FILE)
        .assert_file_change(&repo, CLEAN_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_returns_none_for_clean_worktree_with_subdir(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let worktree_path = clean_worktree_with_subdir.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, "subdir")
        .assert_path_changes(&repo, "subdir/.")
        .assert_path_changes(&repo, SUBDIR_CLEAN_FILE)
        .assert_file_change(&repo, SUBDIR_CLEAN_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_modified_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_file: PathInTempDir,
) {
    let worktree_path = worktree_with_modified_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([MODIFIED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, MODIFIED_FILE)
        .assert_file_change(&repo, MODIFIED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_modified_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let worktree_path = worktree_with_modified_subdir.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([SUBDIR_MODIFIED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, "subdir")
        .assert_path_changes(&repo, "subdir/.")
        .assert_path_changes(&repo, SUBDIR_MODIFIED_FILE)
        .assert_file_change(&repo, SUBDIR_MODIFIED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_staged_file: PathInTempDir,
) {
    let worktree_path = worktree_with_staged_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .staged([STAGED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, STAGED_FILE)
        .assert_file_change(&repo, STAGED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_modified_and_staged_file(
    backend: &dyn VcsBackend,
    worktree_with_modified_and_staged_file: PathInTempDir,
) {
    let worktree_path = worktree_with_modified_and_staged_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([MODIFIED_AND_STAGED_FILE])
        .staged([MODIFIED_AND_STAGED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, MODIFIED_AND_STAGED_FILE)
        .assert_file_change(&repo, MODIFIED_AND_STAGED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_deleted_file: PathInTempDir,
) {
    let worktree_path = worktree_with_deleted_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([DELETED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, DELETED_FILE)
        .assert_file_change(&repo, DELETED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_index_deleted_file(
    backend: &dyn VcsBackend,
    worktree_with_index_deleted_file: PathInTempDir,
) {
    let worktree_path = worktree_with_index_deleted_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .staged([INDEX_DELETED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, INDEX_DELETED_FILE)
        .assert_file_change(&repo, INDEX_DELETED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_untracked_file: PathInTempDir,
) {
    let worktree_path = worktree_with_untracked_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([UNTRACKED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, UNTRACKED_FILE)
        .assert_file_change(&repo, UNTRACKED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_untracked_file_in_subdir(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let worktree_path = worktree_with_untracked_subdir.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([SUBDIR_UNTRACKED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, "subdir")
        .assert_path_changes(&repo, "subdir/.")
        .assert_path_changes(&repo, SUBDIR_UNTRACKED_FILE)
        .assert_file_change(&repo, SUBDIR_UNTRACKED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_conflicted_file_as_dirty_and_staged(
    backend: &dyn VcsBackend,
    worktree_with_conflicted_file: PathInTempDir,
) {
    let worktree_path = worktree_with_conflicted_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([CONFLICTED_FILE])
        .staged([CONFLICTED_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, CONFLICTED_FILE)
        .assert_file_change(&repo, CONFLICTED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_returns_none_for_worktree_with_ignored_file(
    backend: &dyn VcsBackend,
    worktree_with_ignored_file: PathInTempDir,
) {
    let worktree_path = worktree_with_ignored_file.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, IGNORED_FILE)
        .assert_file_change(&repo, IGNORED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_returns_none_for_file_in_ignored_directory_path(
    backend: &dyn VcsBackend,
    worktree_with_ignored_subdir: PathInTempDir,
) {
    let worktree_path = worktree_with_ignored_subdir.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, SUBDIR_IGNORED_FILE)
        .assert_file_change(&repo, SUBDIR_IGNORED_FILE);
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[apply(all_backends)]
#[rstest]
fn query_changes_reports_non_utf8_untracked_file(
    backend: &dyn VcsBackend,
    worktree_with_non_utf8_untracked_file: PathInTempDir,
) {
    let worktree_path = worktree_with_non_utf8_untracked_file.path();
    let repo = open_repo(backend, worktree_path);
    let untracked_file = non_utf8_untracked_file();
    AssertRepositoryChanges::default()
        .dirty([&untracked_file])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, &untracked_file)
        .assert_file_change(&repo, &untracked_file);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_mixed_changes(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let worktree_path = worktree_with_mixed_changes.path();
    let repo = open_repo(backend, worktree_path);
    let changes = AssertRepositoryChanges::default()
        .dirty([
            MODIFIED_FILE,
            MODIFIED_AND_STAGED_FILE,
            DELETED_FILE,
            UNTRACKED_FILE,
        ])
        .staged([STAGED_FILE, MODIFIED_AND_STAGED_FILE, INDEX_DELETED_FILE]);
    changes
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".");
    let queries = [
        CLEAN_FILE,
        MODIFIED_FILE,
        STAGED_FILE,
        MODIFIED_AND_STAGED_FILE,
        DELETED_FILE,
        INDEX_DELETED_FILE,
        UNTRACKED_FILE,
        IGNORED_FILE,
    ];
    for query in queries {
        changes
            .with_filtered(query, |c| {
                c.assert_path_changes(&repo, query);
            })
            .assert_file_change(&repo, query);
    }
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_deleted_file_under_missing_directory_prefix(
    backend: &dyn VcsBackend,
    worktree_with_deleted_directory: PathInTempDir,
) {
    let worktree_path = worktree_with_deleted_directory.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([DELETED_DIR_FILE])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, "deleted_dir")
        .assert_path_changes(&repo, "deleted_dir/.")
        .assert_path_changes(&repo, DELETED_DIR_FILE);
}

#[apply(all_backends)]
#[rstest]
fn query_changes_reports_only_changes_under_queried_directory_or_queried_file(
    backend: &dyn VcsBackend,
    worktree_with_root_and_subdir_changes: PathInTempDir,
) {
    let worktree_path = worktree_with_root_and_subdir_changes.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([
            MODIFIED_FILE,
            SUBDIR_MODIFIED_FILE,
            SUBDIR1_MODIFIED_FILE,
            UNTRACKED_FILE,
            SUBDIR_UNTRACKED_FILE,
            SUBDIR1_UNTRACKED_FILE,
        ])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .with_filtered("subdir", |c| {
            c.assert_path_changes(&repo, "subdir")
                .assert_path_changes(&repo, "subdir/")
                .assert_path_changes(&repo, "subdir/.");
        })
        .with_filtered("subdir1", |c| {
            c.assert_path_changes(&repo, "subdir1")
                .assert_path_changes(&repo, "subdir1/")
                .assert_path_changes(&repo, "subdir1/.");
        })
        .with_filtered(SUBDIR_MODIFIED_FILE, |c| {
            c.assert_path_changes(&repo, SUBDIR_MODIFIED_FILE);
        })
        .with_filtered(SUBDIR1_MODIFIED_FILE, |c| {
            c.assert_path_changes(&repo, SUBDIR1_MODIFIED_FILE);
        });
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[apply(all_backends)]
#[rstest]
fn query_changes_reports_non_utf8_untracked_file_in_aggregate_and_direct_file_query(
    backend: &dyn VcsBackend,
    worktree_with_non_utf8_untracked_file: PathInTempDir,
) {
    let worktree_path = worktree_with_non_utf8_untracked_file.path();
    let repo = open_repo(backend, worktree_path);
    let untracked_file = non_utf8_untracked_file();
    AssertRepositoryChanges::default()
        .dirty([&untracked_file])
        .assert_repository_changes(&repo)
        .assert_path_changes(&repo, "")
        .assert_path_changes(&repo, ".")
        .assert_path_changes(&repo, &untracked_file);
}

#[apply(all_backends)]
#[rstest]
fn path_changes_rejects_non_existent_path(
    backend: &dyn VcsBackend,
    worktree_with_untracked_subdir: PathInTempDir,
) {
    let worktree_path = worktree_with_untracked_subdir.path();
    let repo = open_repo(backend, worktree_path);
    for query in ["xxx", "subdir/xxx.txt"] {
        assert_path_changes_err_matches!(&repo, query, PathNotFound { .. });
    }
}

#[apply(all_backends)]
#[rstest]
fn path_changes_treats_directory_path_as_literal_pathspec(
    backend: &dyn VcsBackend,
    worktree_with_literal_and_glob_matching_subdirs: PathInTempDir,
) {
    let worktree_path = worktree_with_literal_and_glob_matching_subdirs.path();
    let repo = open_repo(backend, worktree_path);
    AssertRepositoryChanges::default()
        .dirty([
            LITERAL_SUBDIR_MODIFIED_FILE,
            GLOB_MATCHING_SUBDIR_MODIFIED_FILE,
        ])
        .assert_repository_changes(&repo)
        .with_filtered("subdir[1]", |c| {
            c.assert_path_changes(&repo, "subdir[1]");
        });
}

#[cfg(unix)]
#[apply(all_backends)]
#[rstest]
fn file_change_resolves_symlink(backend: &dyn VcsBackend, worktree_with_symlink: PathInTempDir) {
    let worktree_path = worktree_with_symlink.path();
    let repo = open_repo(backend, worktree_path);
    AssertFileChange::new(CLEAN_FILE)
        .dirty()
        .assert_file_change(&repo, SYMLINK_FILE);
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_ambiguous_file_path_error_for_missing_directory_with_deleted_file_under_it(
    backend: &dyn VcsBackend,
    worktree_with_deleted_directory: PathInTempDir,
) {
    let worktree_path = worktree_with_deleted_directory.path();
    let repo = open_repo(backend, worktree_path);
    assert_file_change_err_matches!(&repo, "deleted_dir", AmbiguousFilePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_ambiguous_file_path_error_for_missing_directory_with_multiple_deleted_files_under_it(
    backend: &dyn VcsBackend,
    worktree_with_deleted_directory_multiple_files: PathInTempDir,
) {
    let worktree_path = worktree_with_deleted_directory_multiple_files.path();
    let repo = open_repo(backend, worktree_path);
    assert_file_change_err_matches!(&repo, "deleted_dir", AmbiguousFilePath { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_non_existent_file(backend: &dyn VcsBackend, clean_worktree: PathInTempDir) {
    let worktree_path = clean_worktree.path();
    let repo = open_repo(backend, worktree_path);
    assert_file_change_err_matches!(&repo, "non_existent_file.txt", PathNotFound { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_returns_canonicalized_path(
    backend: &dyn VcsBackend,
    worktree_with_modified_subdir: PathInTempDir,
) {
    let worktree_path = worktree_with_modified_subdir.path();
    let dir_name = worktree_path.file_name().unwrap().to_str().unwrap();
    let repo = open_repo(backend, worktree_path);

    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, format!("subdir//{MODIFIED_FILE}"));
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, format!("./{SUBDIR_MODIFIED_FILE}"));
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, format!("subdir/./{MODIFIED_FILE}"));
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, format!("../{dir_name}/{SUBDIR_MODIFIED_FILE}"));
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, format!("subdir/../{SUBDIR_MODIFIED_FILE}"));
    AssertFileChange::new(SUBDIR_MODIFIED_FILE)
        .dirty()
        .assert_file_change(&repo, SUBDIR_MODIFIED_FILE);
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_empty_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let worktree_path = clean_worktree_with_subdir.path();
    let repo = open_repo(backend, worktree_path);
    assert_file_change_err_matches!(&repo, "", PathNotAFile { .. });
}

#[apply(all_backends)]
#[rstest]
fn file_change_rejects_directory_path(
    backend: &dyn VcsBackend,
    clean_worktree_with_subdir: PathInTempDir,
) {
    let worktree_path = clean_worktree_with_subdir.path();
    let repo = open_repo(backend, worktree_path);
    assert_file_change_err_matches!(&repo, "subdir", PathNotAFile { .. });
}

#[apply(all_backends)]
#[rstest]
fn repository_changes_and_file_change_agree_for_reported_paths(
    backend: &dyn VcsBackend,
    worktree_with_mixed_changes: PathInTempDir,
) {
    let worktree_path = worktree_with_mixed_changes.path();
    let repo = open_repo(backend, worktree_path);
    let repo_changes = assert_repository_changes_ok(&repo).unwrap();

    let wt_paths = [
        CLEAN_FILE,
        MODIFIED_FILE,
        STAGED_FILE,
        MODIFIED_AND_STAGED_FILE,
        DELETED_FILE,
        INDEX_DELETED_FILE,
        UNTRACKED_FILE,
        IGNORED_FILE,
    ];

    let mut dirty_count = 0;
    let mut staged_count = 0;

    let repo_dirty_wt_paths = repo_changes
        .dirty_files()
        .map(FileChange::wt_path)
        .collect::<Vec<_>>();
    let repo_staged_wt_paths = repo_changes
        .staged_files()
        .map(FileChange::wt_path)
        .collect::<Vec<_>>();

    for wt_path in &wt_paths {
        let wt_path = Path::new(wt_path);
        let file_change = assert_file_change_ok(&repo, wt_path);
        let mut expected = AssertFileChange::new(wt_path);
        if repo_dirty_wt_paths.contains(&wt_path) {
            dirty_count += 1;
            expected = expected.dirty();
        }
        if repo_staged_wt_paths.contains(&wt_path) {
            staged_count += 1;
            expected = expected.staged();
        }
        expected.assert(file_change);
    }

    assert_eq!(repo_dirty_wt_paths.len(), dirty_count);
    assert_eq!(repo_staged_wt_paths.len(), staged_count);
}
