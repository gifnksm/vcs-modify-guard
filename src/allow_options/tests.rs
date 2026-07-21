use super::*;
use crate::repository::FileChange;

const WORKTREE: &str = "repo";

fn modified_file(wt_path: &str) -> FileChange {
    FileChange {
        wt_path: wt_path.into(),
        modified: true,
        staged: false,
        untracked: false,
    }
}

fn staged_file(wt_path: &str) -> FileChange {
    FileChange {
        wt_path: wt_path.into(),
        modified: false,
        staged: true,
        untracked: false,
    }
}

fn modified_and_staged_file(wt_path: &str) -> FileChange {
    FileChange {
        wt_path: wt_path.into(),
        modified: true,
        staged: true,
        untracked: false,
    }
}

fn untracked_file(wt_path: &str) -> FileChange {
    FileChange {
        wt_path: wt_path.into(),
        modified: false,
        staged: false,
        untracked: true,
    }
}

#[derive(Clone, Debug)]
struct StubRepo {
    worktree: PathBuf,
    resolve_path_result: Option<PathBuf>,
    expected_path_changes_wt_path: Option<PathBuf>,
    path_changes_result: Option<RepositoryChanges>,
    repository_changes_result: Option<RepositoryChanges>,
}

impl Default for StubRepo {
    fn default() -> Self {
        Self {
            worktree: PathBuf::from(WORKTREE),
            resolve_path_result: None,
            expected_path_changes_wt_path: None,
            path_changes_result: None,
            repository_changes_result: None,
        }
    }
}

impl StubRepo {
    fn with_resolve_path_result<P>(mut self, wt_path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.resolve_path_result = Some(wt_path.into());
        self
    }

    fn with_expected_path_changes_wt_path<P>(mut self, wt_path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.expected_path_changes_wt_path = Some(wt_path.into());
        self
    }

    fn with_path_changes<I>(mut self, files: I) -> Self
    where
        I: IntoIterator<Item = FileChange>,
    {
        self.path_changes_result = RepositoryChanges::new(files);
        self
    }

    fn with_repository_changes<I>(mut self, files: I) -> Self
    where
        I: IntoIterator<Item = FileChange>,
    {
        self.repository_changes_result = RepositoryChanges::new(files);
        self
    }
}

impl AllowOptionsRepository for StubRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn resolve_path(&self, path: &Path) -> Result<PathBuf, ModifyGuardError> {
        Ok(self
            .resolve_path_result
            .clone()
            .unwrap_or_else(|| path.to_path_buf()))
    }

    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        if let Some(expected) = &self.expected_path_changes_wt_path {
            assert_eq!(wt_path, expected);
        }
        Ok(self.path_changes_result.clone())
    }

    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        Ok(self.repository_changes_result.clone())
    }
}

#[derive(Debug, Default)]
struct StubBackend {
    repo: Option<StubRepo>,
}

impl StubBackend {
    fn with_repo(mut self, repo: StubRepo) -> Self {
        self.repo = Some(repo);
        self
    }
}

impl AllowOptionsBackend for StubBackend {
    type Repo = StubRepo;

    fn discover(&self, _path: &Path) -> Result<Option<Self::Repo>, ModifyGuardError> {
        Ok(self.repo.clone())
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Consuming the value keeps call sites concise in these assertion helpers"
)]
#[track_caller]
fn assert_allowed(result: CheckResult) {
    assert!(matches!(result, CheckResult::Allowed));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Consuming the value keeps call sites concise in these assertion helpers"
)]
#[track_caller]
fn assert_blocked_by_no_vcs(result: CheckResult) {
    assert!(matches!(result, CheckResult::BlockedByNoVcs));
}

#[track_caller]
fn assert_blocked_by_dirty(
    result: CheckResult,
    expected_dirty_files: &[&str],
    expected_staged_files: &[&str],
) {
    let CheckResult::BlockedByDirty {
        worktree,
        dirty_files,
        staged_files,
    } = result
    else {
        panic!("expected BlockedByDirty");
    };
    assert_eq!(worktree, PathBuf::from(WORKTREE));
    assert_eq!(
        dirty_files,
        expected_dirty_files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        staged_files,
        expected_staged_files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    );
}

#[track_caller]
fn assert_blocked_by_staged(result: CheckResult, expected_staged_files: &[&str]) {
    let CheckResult::BlockedByStaged {
        worktree,
        staged_files,
    } = result
    else {
        panic!("expected BlockedByStaged");
    };
    assert_eq!(worktree, PathBuf::from(WORKTREE));
    assert_eq!(
        staged_files,
        expected_staged_files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn check_safe_to_modify_returns_allowed_when_allow_no_vcs_is_enabled() {
    let backend = StubBackend::default();

    let result = AllowOptions::new()
        .allow_no_vcs(true)
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_allowed(result);
}

#[test]
fn check_safe_to_modify_returns_blocked_by_no_vcs_when_repository_is_not_found() {
    let backend = StubBackend::default();

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_blocked_by_no_vcs(result);
}

#[test]
fn check_safe_to_modify_returns_allowed_for_clean_path() {
    let backend = StubBackend::default().with_repo(StubRepo::default());

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_allowed(result);
}

#[test]
fn check_safe_to_modify_returns_blocked_by_staged_for_staged_only_path() {
    let backend = StubBackend::default()
        .with_repo(StubRepo::default().with_path_changes([staged_file("staged.txt")]));

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_blocked_by_staged(result, &["staged.txt"]);
}

#[test]
fn check_safe_to_modify_returns_allowed_for_staged_only_path_when_allow_staged_is_enabled() {
    let backend = StubBackend::default()
        .with_repo(StubRepo::default().with_path_changes([staged_file("staged.txt")]));

    let result = AllowOptions::new()
        .allow_staged(true)
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_allowed(result);
}

#[test]
fn check_safe_to_modify_returns_blocked_by_dirty_for_dirty_only_path() {
    let backend = StubBackend::default().with_repo(StubRepo::default().with_path_changes([
        modified_file("modified.txt"),
        untracked_file("untracked.txt"),
    ]));

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_blocked_by_dirty(result, &["modified.txt", "untracked.txt"], &[]);
}

#[test]
fn check_safe_to_modify_returns_blocked_by_dirty_with_dirty_and_staged_files() {
    let backend = StubBackend::default().with_repo(StubRepo::default().with_path_changes([
        modified_file("a-modified.txt"),
        staged_file("b-staged.txt"),
        modified_and_staged_file("c-modified-and-staged.txt"),
        untracked_file("d-untracked.txt"),
    ]));

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_blocked_by_dirty(
        result,
        &[
            "a-modified.txt",
            "c-modified-and-staged.txt",
            "d-untracked.txt",
        ],
        &["b-staged.txt", "c-modified-and-staged.txt"],
    );
}

#[test]
fn check_safe_to_modify_returns_blocked_by_dirty_without_staged_files_when_allow_staged_is_enabled()
{
    let backend = StubBackend::default().with_repo(StubRepo::default().with_path_changes([
        modified_file("a-modified.txt"),
        staged_file("b-staged.txt"),
        modified_and_staged_file("c-modified-and-staged.txt"),
    ]));

    let result = AllowOptions::new()
        .allow_staged(true)
        .check_safe_to_modify_with_backend("target", &backend)
        .unwrap();

    assert_blocked_by_dirty(
        result,
        &["a-modified.txt", "c-modified-and-staged.txt"],
        &[],
    );
}

#[test]
fn check_safe_to_modify_checks_only_the_queried_path_by_default() {
    let backend = StubBackend::default().with_repo(
        StubRepo::default().with_repository_changes([modified_file("root-modified.txt")]),
    );

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("subdir", &backend)
        .unwrap();

    assert_allowed(result);
}

#[test]
fn check_safe_to_modify_resolves_path_before_querying_path_changes() {
    let backend = StubBackend::default().with_repo(
        StubRepo::default()
            .with_resolve_path_result("resolved/subdir")
            .with_expected_path_changes_wt_path("resolved/subdir"),
    );

    let result = AllowOptions::new()
        .check_safe_to_modify_with_backend("input/subdir", &backend)
        .unwrap();

    assert_allowed(result);
}

#[test]
fn check_safe_to_modify_checks_the_entire_repository_when_enabled() {
    let backend = StubBackend::default().with_repo(
        StubRepo::default().with_repository_changes([modified_file("root-modified.txt")]),
    );

    let result = AllowOptions::new()
        .check_entire_repository(true)
        .check_safe_to_modify_with_backend("subdir", &backend)
        .unwrap();

    assert_blocked_by_dirty(result, &["root-modified.txt"], &[]);
}
