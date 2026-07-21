use std::{
    fmt,
    path::{Path, PathBuf},
};

use snafu::{IntoError as _, ResultExt as _, Snafu};

use super::VcsRepository;
use crate::{
    error::{self, ModifyGuardError},
    repository::{FileChange, RepositoryChanges},
    util::{self, NormalizedPath},
    vcs::VcsBackend,
};

pub(super) const BACKEND: Libgit2Backend = Libgit2Backend;

#[derive(Debug)]
pub(super) struct Libgit2Backend;

/// Errors returned by `libgit2` backend operations.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Libgit2BackendError {
    /// Searching for a Git repository failed.
    #[snafu(display("failed while searching for a git repository at or above path: {}", path.display()))]
    Discover {
        /// The path that was being searched for a Git repository.
        path: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
    /// Opening a Git repository failed.
    #[snafu(display("failed to open git repository at path: {}", path.display()))]
    Open {
        /// The path that was being opened as a Git repository.
        path: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
    /// Querying repository changes failed.
    #[snafu(display("failed to query git repository changes for worktree: {}", worktree.display()))]
    QueryRepositoryChanges {
        /// The worktree of the Git repository.
        worktree: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
    /// Querying file change failed.
    #[snafu(display("failed to query git file change for worktree-relative path: {}", wt_path.display()))]
    QueryFileChange {
        /// The worktree-relative path of the file whose change was being retrieved.
        wt_path: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
}

impl From<Libgit2BackendError> for ModifyGuardError {
    #[inline]
    fn from(source: Libgit2BackendError) -> Self {
        Self::Backend {
            source: source.into(),
        }
    }
}

impl VcsBackend for Libgit2Backend {
    fn discover(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
        util::ensure_path_exists(path)?;
        let repo = match git2::Repository::discover(path) {
            Ok(repo) => repo,
            Err(source) if source.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(source) => {
                return Err(DiscoverSnafu { path }.into_error(source).into());
            }
        };
        let Some(worktree) = repo.workdir() else {
            return Err(error::RepositoryWithoutWorktreeSnafu { path: repo.path() }.build());
        };
        let worktree = worktree.to_owned();
        Ok(Some(Box::new(Libgit2Repository { repo, worktree })))
    }

    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
        util::ensure_path_is_directory(path)?;
        let repo = match git2::Repository::open(path) {
            Ok(repo) => repo,
            Err(source) if source.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(source) => {
                return Err(OpenSnafu { path }.into_error(source).into());
            }
        };
        let Some(worktree) = repo.workdir() else {
            return Err(error::RepositoryWithoutWorktreeSnafu { path: repo.path() }.build());
        };
        let worktree = worktree.to_owned();
        Ok(Some(Box::new(Libgit2Repository { repo, worktree })))
    }
}

struct Libgit2Repository {
    repo: git2::Repository,
    worktree: PathBuf,
}

impl fmt::Debug for Libgit2Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Libgit2Repository")
            .field("repo", &"<git2::Repository>")
            .field("worktree", &self.worktree)
            .finish()
    }
}

impl VcsRepository for Libgit2Repository {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        self.directory_path_changes(None)
    }

    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        let wt_path = util::normalize_worktree_path(&self.worktree, wt_path)?;
        let is_dir = match &wt_path {
            NormalizedPath::Existing(wt_path) => {
                let fs_path = self.worktree.join(wt_path);
                let metadata = util::read_path_metadata(&fs_path)?;
                metadata.is_dir()
            }
            NormalizedPath::Missing(_) => true,
        };
        if wt_path.as_path().as_os_str().is_empty() {
            return self.directory_path_changes(None);
        }
        if is_dir {
            return self.directory_path_changes(Some(&wt_path));
        }
        let change = self.file_path_change(wt_path)?;
        Ok(change.and_then(|change| RepositoryChanges::new([change])))
    }

    fn file_change(&self, wt_path: &Path) -> Result<Option<FileChange>, ModifyGuardError> {
        let wt_path = util::normalize_worktree_path(&self.worktree, wt_path)?;
        match &wt_path {
            NormalizedPath::Existing(wt_path) => {
                let fs_path = self.worktree.join(wt_path);
                util::ensure_path_is_file(&fs_path)?;
            }
            NormalizedPath::Missing(_) => {}
        }
        self.file_path_change(wt_path)
    }
}

impl Libgit2Repository {
    fn directory_path_changes(
        &self,
        wt_path: Option<&NormalizedPath>,
    ) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        let mut repo_opts = git2::StatusOptions::new();
        if let Some(wt_path) = wt_path {
            repo_opts.pathspec(wt_path.as_path());
            repo_opts.disable_pathspec_match(true);
        }
        repo_opts.include_untracked(true);
        repo_opts.recurse_untracked_dirs(true);
        let entries =
            self.repo
                .statuses(Some(&mut repo_opts))
                .context(QueryRepositoryChangesSnafu {
                    worktree: &self.worktree,
                })?;
        let mut file_entries = entries
            .iter()
            .filter_map(|entry| {
                // Match `cargo fix`: ignore status entries whose paths cannot be represented as UTF-8.
                let wt_path = entry.path().ok()?;
                StatusFlags::from(entry.status()).build(wt_path)
            })
            .peekable();

        if file_entries.peek().is_none()
            && let Some(NormalizedPath::Missing(wt_path)) = &wt_path
        {
            return Err(error::PathNotFoundSnafu { path: wt_path }.build());
        }

        Ok(RepositoryChanges::new(file_entries))
    }

    fn file_path_change(
        &self,
        wt_path: NormalizedPath,
    ) -> Result<Option<FileChange>, ModifyGuardError> {
        let status = match self.repo.status_file(wt_path.as_path()) {
            Ok(status) => status,
            Err(source) if source.code() == git2::ErrorCode::NotFound => {
                match &wt_path {
                    NormalizedPath::Existing(wt_path) => {
                        // At this point `wt_path` has already been resolved to an
                        // existing file within the worktree, so `NotFound` means the
                        // file is untracked by Git rather than missing from disk.
                        return Ok(StatusFlags::untracked().build(wt_path));
                    }
                    NormalizedPath::Missing(wt_path) => {
                        return Err(error::PathNotFoundSnafu { path: wt_path }.build());
                    }
                }
            }
            Err(source) => {
                return Err(QueryFileChangeSnafu { wt_path }.into_error(source).into());
            }
        };
        Ok(StatusFlags::from(status).build(wt_path))
    }
}

#[derive(Debug, Clone, Copy)]
struct StatusFlags {
    modified: bool,
    staged: bool,
    untracked: bool,
}

impl From<git2::Status> for StatusFlags {
    fn from(status: git2::Status) -> Self {
        if status.is_wt_new() {
            return Self::untracked();
        }
        let modified = status.is_wt_modified()
            || status.is_wt_deleted()
            || status.is_wt_renamed()
            || status.is_wt_typechange();
        let staged = status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange();
        Self::tracked(modified, staged)
    }
}

impl StatusFlags {
    fn untracked() -> Self {
        Self {
            modified: false,
            staged: false,
            untracked: true,
        }
    }

    fn tracked(modified: bool, staged: bool) -> Self {
        Self {
            modified,
            staged,
            untracked: false,
        }
    }

    fn build<P>(self, wt_path: P) -> Option<FileChange>
    where
        P: Into<PathBuf>,
    {
        let Self {
            modified,
            staged,
            untracked,
        } = self;
        if !modified && !staged && !untracked {
            return None;
        }

        let wt_path = wt_path.into();
        Some(FileChange {
            wt_path,
            modified,
            staged,
            untracked,
        })
    }
}
