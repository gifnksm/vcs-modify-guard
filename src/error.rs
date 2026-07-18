use std::{error::Error, io, path::PathBuf};

use snafu::Snafu;

/// Errors returned by `vcs-status` operations.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub(crate)))]
pub enum VcsStatusError {
    /// The specified path does not refer to a repository supported by the
    /// enabled backends.
    #[snafu(display("not a VCS repository: {}", path.display()))]
    NotARepository {
        /// The path that was rejected.
        path: PathBuf,
    },
    /// The specified path refers to a repository without a worktree, such as a
    /// Git bare repository.
    #[snafu(display("repository has no worktree: {}", path.display()))]
    RepositoryWithoutWorktree {
        /// The path that was rejected.
        path: PathBuf,
    },
    /// The specified path could not be accessed.
    #[snafu(display("path is inaccessible: {}", path.display()))]
    InaccessiblePath {
        /// The path that was rejected.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The specified path was not found.
    #[snafu(display("path not found: {}", path.display()))]
    PathNotFound {
        /// The path that was rejected.
        path: PathBuf,
    },
    /// The specified path is not a directory.
    #[snafu(display("path is not a directory: {}", path.display()))]
    PathNotADirectory {
        /// The path that was rejected.
        path: PathBuf,
    },
    /// The specified path is not a valid path relative to the repository
    /// worktree.
    #[snafu(display(
        "path is not a valid worktree-relative path: {}",
        path.display()
    ))]
    InvalidWorktreeRelativePath {
        /// The path that was rejected.
        path: PathBuf,
    },
    /// VCS backend error.
    #[snafu(transparent)]
    Backend {
        /// The underlying error returned by the VCS backend.
        source: Box<dyn Error + Send + Sync + 'static>,
    },
}

// assert that VcsStatusError is Send + Sync
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VcsStatusError>();
};
