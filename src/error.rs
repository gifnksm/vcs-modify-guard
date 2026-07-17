use std::path::PathBuf;

use snafu::Snafu;

/// Errors returned by `vcs-status` operations.
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub(crate)))]
pub enum VcsStatusError {
    /// The specified path does not refer to a repository supported by the
    /// enabled backends.
    #[snafu(display("Not a VCS repository: {}", path.display()))]
    NotARepository {
        /// The path that was rejected.
        path: PathBuf,
    },
}
