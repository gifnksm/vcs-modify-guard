use std::path::Path;

use crate::{
    VcsStatusError,
    vcs::{self, VcsRepository},
};

/// A version control repository that can report status relevant to
/// `--allow-*` style checks.
#[derive(Debug)]
pub struct Repository {
    inner: Box<dyn VcsRepository>,
}

impl Repository {
    /// Discovers the repository containing `path`.
    ///
    /// This searches `path` and its parent directories for a repository
    /// supported by one of the enabled backends.
    ///
    /// Returns `Ok(Some(_))` if a supported repository is found, `Ok(None)`
    /// if no supported repository is found, and `Err(_)` if a backend fails
    /// while probing `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if a backend fails while probing `path`.
    #[inline]
    pub fn discover<P>(path: P) -> Result<Option<Self>, VcsStatusError>
    where
        P: AsRef<Path>,
    {
        let Some(inner) = vcs::discover(path.as_ref())? else {
            return Ok(None);
        };
        Ok(Some(Self { inner }))
    }

    /// Opens the repository at `path`.
    ///
    /// Unlike [`Self::discover`], this does not search parent directories.
    /// `path` must identify a repository directly according to the enabled
    /// backend.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` does not refer to a supported repository
    /// or if the backend fails to open it.
    #[inline]
    pub fn open<P>(path: P) -> Result<Self, VcsStatusError>
    where
        P: AsRef<Path>,
    {
        let inner = vcs::open(path.as_ref())?;
        Ok(Self { inner })
    }

    /// Returns the root directory of the repository worktree.
    #[inline]
    #[must_use]
    pub fn workdir(&self) -> &Path {
        self.inner.workdir()
    }

    /// Returns the aggregate status of the repository worktree.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to query the repository status.
    #[inline]
    pub fn status(&self) -> Result<RepositoryStatus, VcsStatusError> {
        self.inner.status()
    }

    /// Returns the status of a single file `path` within the repository.
    ///
    /// A file may be both staged and modified at the same time if it has
    /// staged changes and additional unstaged changes.
    ///
    /// This operation is intended for file paths. It does not perform rename
    /// detection.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` does not identify exactly one file status,
    /// or if the backend fails to query it.
    #[inline]
    pub fn file_status<P>(&self, path: P) -> Result<FileStatus, VcsStatusError>
    where
        P: AsRef<Path>,
    {
        self.inner.file_status(path.as_ref())
    }
}

/// A summary of repository state relevant to `--allow-*` checks.
#[expect(
    missing_copy_implementations,
    reason = "`Copy` is not part of this crate's public API contract"
)]
#[derive(Debug, Clone)]
pub struct RepositoryStatus {
    pub(crate) has_modified_files: bool,
    pub(crate) has_staged_files: bool,
    pub(crate) has_untracked_files: bool,
}

impl RepositoryStatus {
    /// Returns whether the repository has tracked worktree changes.
    ///
    /// This does not include staged changes or untracked files.
    #[inline]
    #[must_use]
    pub fn has_worktree_changes(&self) -> bool {
        self.has_modified_files
    }

    /// Returns whether the repository has staged changes in the index.
    #[inline]
    #[must_use]
    pub fn has_staged_changes(&self) -> bool {
        self.has_staged_files
    }

    /// Returns whether the repository has untracked files.
    #[inline]
    #[must_use]
    pub fn has_untracked_files(&self) -> bool {
        self.has_untracked_files
    }

    /// Returns whether the repository has any worktree, staged, or untracked
    /// changes.
    #[inline]
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.has_worktree_changes() || self.has_staged_changes() || self.has_untracked_files()
    }
}

/// The status of a single file within a repository.
///
/// More than one predicate may return `true` for the same file. For example,
/// a file may have staged changes and additional unstaged modifications.
#[expect(
    missing_copy_implementations,
    reason = "`Copy` is not part of this crate's public API contract"
)]
#[derive(Debug, Clone)]
pub struct FileStatus {
    pub(crate) modified: bool,
    pub(crate) staged: bool,
    pub(crate) untracked: bool,
}

impl FileStatus {
    /// Returns whether the file has tracked worktree changes.
    ///
    /// This does not include staged changes or untracked files.
    #[inline]
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Returns whether the file has staged changes in the index.
    #[inline]
    #[must_use]
    pub fn is_staged(&self) -> bool {
        self.staged
    }

    /// Returns whether the file is untracked.
    #[inline]
    #[must_use]
    pub fn is_untracked(&self) -> bool {
        self.untracked
    }

    /// Returns whether the file is modified, staged, or untracked.
    #[inline]
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.is_modified() || self.is_staged() || self.is_untracked()
    }
}
