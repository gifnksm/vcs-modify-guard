use std::{fmt::Debug, path::Path};

use crate::{
    error::{self, VcsStatusError},
    repository::{FileStatus, RepositoryStatus},
};

#[cfg(feature = "git-libgit2")]
pub use self::git_libgit2::Libgit2BackendError;

#[cfg(feature = "git-libgit2")]
mod git_libgit2;
#[cfg(test)]
mod tests;

trait VcsBackend: Debug + Send + Sync {
    fn discover(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError>;
    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError>;
}

static BACKENDS: &[&dyn VcsBackend] = &[
    #[cfg(feature = "git-libgit2")]
    &git_libgit2::BACKEND,
];

pub(crate) fn discover(path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError> {
    for backend in BACKENDS {
        if let Some(repo) = backend.discover(path)? {
            return Ok(Some(repo));
        }
    }
    Ok(None)
}

pub(crate) fn open(path: &Path) -> Result<Box<dyn VcsRepository>, VcsStatusError> {
    for backend in BACKENDS {
        if let Some(repo) = backend.open(path)? {
            return Ok(repo);
        }
    }
    Err(error::NotARepositorySnafu { path }.build())
}

pub(crate) trait VcsRepository: Debug {
    fn worktree(&self) -> &Path;
    fn status(&self) -> Result<RepositoryStatus, VcsStatusError>;
    fn file_status(&self, path: &Path) -> Result<FileStatus, VcsStatusError>;
}

// assert that VcsRepository is dyn safe
const _: Option<&dyn VcsRepository> = None;
