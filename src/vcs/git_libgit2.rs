use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

use snafu::{IntoError as _, ResultExt as _, Snafu};

use crate::{
    error::{self, VcsStatusError},
    repository::{FileStatus, RepositoryStatus},
    util,
    vcs::VcsBackend,
};

use super::VcsRepository;

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
    /// Querying repository status failed.
    #[snafu(display("failed to query git repository status for worktree: {}", worktree.display()))]
    QueryRepositoryStatus {
        /// The worktree of the Git repository.
        worktree: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
    /// Querying file status failed.
    #[snafu(display("failed to query git file status for path: {}", path.display()))]
    QueryFileStatus {
        /// The path of the file whose status was being retrieved.
        path: PathBuf,
        /// The underlying error from `libgit2`.
        source: git2::Error,
    },
}

impl From<Libgit2BackendError> for VcsStatusError {
    #[inline]
    fn from(source: Libgit2BackendError) -> Self {
        Self::Backend {
            source: source.into(),
        }
    }
}

impl VcsBackend for Libgit2Backend {
    fn discover(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError> {
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

    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError> {
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

    fn status(&self) -> Result<RepositoryStatus, VcsStatusError> {
        let mut repo_opts = git2::StatusOptions::new();
        repo_opts.include_ignored(true);
        repo_opts.include_untracked(true);
        let statuses =
            self.repo
                .statuses(Some(&mut repo_opts))
                .context(QueryRepositoryStatusSnafu {
                    worktree: &self.worktree,
                })?;

        let mut modified = BTreeSet::new();
        let mut staged = BTreeSet::new();
        let mut untracked = BTreeSet::new();
        for entry in statuses.iter() {
            let Ok(path) = entry.path() else {
                // Match `cargo fix`: ignore status entries whose paths cannot be
                // represented as UTF-8 when building the returned path sets.
                continue;
            };
            let status = file_status(entry.status());
            if status.modified {
                modified.insert(path.into());
            }
            if status.staged {
                staged.insert(path.into());
            }
            if status.untracked {
                untracked.insert(path.into());
            }
        }
        Ok(RepositoryStatus {
            modified,
            staged,
            untracked,
        })
    }

    fn file_status(&self, path: &Path) -> Result<FileStatus, VcsStatusError> {
        let path = util::normalize_worktree_relative_path(path)?;
        let status = match self.repo.status_file(&path) {
            Ok(status) => status,
            Err(source) if source.code() == git2::ErrorCode::NotFound => {
                // Distinguish an existing untracked file from a missing path.
                let fs_path = self.worktree.join(&path);
                util::ensure_path_exists(&fs_path)?;
                return Ok(FileStatus::untracked());
            }
            Err(source) => {
                return Err(QueryFileStatusSnafu { path }.into_error(source).into());
            }
        };
        Ok(file_status(status))
    }
}

fn file_status(status: git2::Status) -> FileStatus {
    if status.is_ignored() {
        return FileStatus::ignored();
    }
    if status.is_wt_new() {
        return FileStatus::untracked();
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
    FileStatus::tracked(modified, staged)
}
