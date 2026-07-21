use std::{fmt::Debug, path::Path};

#[cfg(feature = "git-libgit2")]
pub use self::git_libgit2::Libgit2BackendError;
use crate::{
    error::{self, ModifyGuardError},
    repository::{FileChange, RepositoryChanges},
};

#[cfg(feature = "git-libgit2")]
mod git_libgit2;
#[cfg(test)]
mod tests;

trait VcsBackend: Debug + Send + Sync {
    fn discover(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError>;
    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError>;
}

static BACKENDS: &[&dyn VcsBackend] = &[
    #[cfg(feature = "git-libgit2")]
    &git_libgit2::BACKEND,
];

pub(crate) fn discover(path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
    for backend in BACKENDS {
        if let Some(repo) = backend.discover(path)? {
            return Ok(Some(repo));
        }
    }
    Ok(None)
}

pub(crate) fn open(path: &Path) -> Result<Box<dyn VcsRepository>, ModifyGuardError> {
    for backend in BACKENDS {
        if let Some(repo) = backend.open(path)? {
            return Ok(repo);
        }
    }
    Err(error::NotARepositorySnafu { path }.build())
}

pub(crate) trait VcsRepository: Debug {
    fn worktree(&self) -> &Path;
    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError>;
    fn path_changes(&self, path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError>;
    fn file_change(&self, path: &Path) -> Result<Option<FileChange>, ModifyGuardError>;
}

// assert that VcsRepository is dyn safe
const _: Option<&dyn VcsRepository> = None;
