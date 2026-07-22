//! Lower-level repository change query APIs.
//!
//! Most users should start with [`crate::AllowOptions`], which implements the
//! crate's built-in `--allow-*` safe-to-modify policy.
//!
//! This module provides the [`Repository`] type for tools that need to
//! discover a VCS repository and inspect whether files are dirty and/or staged
//! directly in order to implement custom policy logic. Dirty files include
//! modified tracked files and untracked files.
//!
//! Paths returned by change queries are relative to the repository worktree.
//! Query methods that take a `wt_path` argument interpret it relative to that
//! worktree. If you start with an absolute path or a path relative to the
//! current working directory, use [`Repository::resolve_path`] first.
//!
//! Start with [`Repository::discover`] or [`Repository::open`], then query
//! repository-wide changes directly or resolve a path and query it within the
//! worktree.
//!
//! # Example
//!
//! The following example shows how to implement a custom safe-to-modify policy
//! by querying repository changes directly.
//!
//! ```no_run
//! use std::{error::Error, path::Path};
//!
//! use vcs_modify_guard::repository::Repository;
//!
//! struct PolicyOptions {
//!     allow_no_vcs: bool,
//!     allow_dirty: bool,
//!     allow_staged: bool,
//! }
//!
//! fn ensure_safe_to_modify(target: &Path, options: &PolicyOptions) -> Result<(), Box<dyn Error>> {
//!     // Match `cargo fix` exactly:
//!     // - `--allow-no-vcs` allows running even when no repository is found.
//!     // - `--allow-dirty` allows worktree changes, staged changes, and
//!     //   untracked files.
//!     // - `--allow-staged` allows staged changes, but still rejects
//!     //   worktree changes and untracked files.
//!     if options.allow_no_vcs {
//!         return Ok(());
//!     }
//!
//!     let Some(repo) = Repository::discover(target)? else {
//!         return Err("no VCS found for the target path".into());
//!     };
//!
//!     let Some(changes) = repo.repository_changes()? else {
//!         return Ok(());
//!     };
//!
//!     if options.allow_dirty {
//!         return Ok(());
//!     }
//!
//!     if changes.has_dirty_files() {
//!         return Err("the repository containing the target path has uncommitted changes".into());
//!     }
//!
//!     if options.allow_staged {
//!         return Ok(());
//!     }
//!
//!     if changes.has_staged_files() {
//!         return Err("the repository containing the target path has staged changes".into());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! See the `repository` example for a complete command-line application.

use std::{
    path::{Path, PathBuf},
    slice,
};

use crate::{
    ModifyGuardError,
    vcs::{self, VcsRepository},
};

#[cfg(test)]
mod tests;

/// A lower-level handle for querying changes in a VCS repository worktree.
///
/// Most users should start with [`crate::AllowOptions`], which implements the
/// crate's built-in `--allow-*` safe-to-modify policy.
///
/// Use this type when you need to discover a repository and inspect whether
/// files are dirty and/or staged directly in order to implement custom policy
/// logic. Dirty files include modified tracked files and untracked files.
///
/// Repositories without a worktree, such as Git bare repositories, are not
/// represented by this type.
///
/// # Example
///
/// The following example shows how to implement a custom safe-to-modify
/// policy by querying repository changes directly.
///
/// ```no_run
/// use std::{error::Error, path::Path};
///
/// use vcs_modify_guard::repository::Repository;
///
/// struct PolicyOptions {
///     allow_no_vcs: bool,
///     allow_dirty: bool,
///     allow_staged: bool,
/// }
///
/// fn ensure_safe_to_modify(target: &Path, options: &PolicyOptions) -> Result<(), Box<dyn Error>> {
///     // Match `cargo fix` exactly:
///     // - `--allow-no-vcs` allows running even when no repository is found.
///     // - `--allow-dirty` allows worktree changes, staged changes, and
///     //   untracked files.
///     // - `--allow-staged` allows staged changes, but still rejects
///     //   worktree changes and untracked files.
///     if options.allow_no_vcs {
///         return Ok(());
///     }
///
///     let Some(repo) = Repository::discover(target)? else {
///         return Err("no VCS found for the target path".into());
///     };
///
///     let Some(changes) = repo.repository_changes()? else {
///         return Ok(());
///     };
///
///     if options.allow_dirty {
///         return Ok(());
///     }
///
///     if changes.has_dirty_files() {
///         return Err("the repository containing the target path has uncommitted changes".into());
///     }
///
///     if options.allow_staged {
///         return Ok(());
///     }
///
///     if changes.has_staged_files() {
///         return Err("the repository containing the target path has staged changes".into());
///     }
///
///     Ok(())
/// }
/// ```
///
/// See the `repository` example for a complete command-line application.
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
    /// Returns `Ok(Some(_))` if a supported repository worktree is found, or
    /// `Ok(None)` if no supported repository is found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - a backend fails while probing `path`
    /// - the discovered repository does not provide a worktree for file
    ///   change checks
    #[inline]
    pub fn discover<P>(path: P) -> Result<Option<Self>, ModifyGuardError>
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
    /// Returns an error if:
    ///
    /// - `path` does not refer to a supported repository worktree
    /// - the backend fails to open it
    #[inline]
    pub fn open<P>(path: P) -> Result<Self, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        let inner = vcs::open(path.as_ref())?;
        Ok(Self { inner })
    }

    /// Returns the root directory of the repository worktree.
    #[inline]
    #[must_use]
    pub fn worktree(&self) -> &Path {
        self.inner.worktree()
    }

    /// Resolves a path to a repository worktree-relative path.
    ///
    /// This follows symlinks and canonicalizes the existing prefix of `path`.
    /// The returned path is relative to [`Self::worktree`].
    ///
    /// If `path` does not exist in the worktree, this method may still succeed
    /// when the missing path is lexically within the worktree, such as a path
    /// to a deleted tracked file.
    ///
    /// Use this when you start with an absolute path or a path relative to the
    /// current working directory and need the corresponding worktree-relative
    /// path for [`Self::path_changes`] or [`Self::file_change`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - `path` does not resolve to a path within [`Self::worktree`]
    /// - `path` could not be resolved to a canonical path for any other reason
    #[inline]
    pub fn resolve_path<P>(&self, path: P) -> Result<PathBuf, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        self.inner.resolve_path(path.as_ref())
    }

    /// Returns the aggregate file changes in the repository worktree.
    ///
    /// Returns `Ok(None)` if the repository has no dirty or staged files.
    /// Clean tracked files are not included in this aggregate
    /// change set. Files ignored by the VCS are also omitted because this
    /// crate is intended for `--allow-*` style checks, which treat them the
    /// same as clean files.
    ///
    /// Paths in the returned changes are relative to [`Self::worktree`].
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to query repository changes.
    #[inline]
    pub fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        self.inner.repository_changes()
    }

    /// Returns the aggregate file changes for the worktree-relative `wt_path`
    /// within the repository.
    ///
    /// If `wt_path` resolves to a file path, the returned change set contains at
    /// most that file. If `wt_path` resolves to a directory path, the returned
    /// change set contains changes for files under that directory. The
    /// repository worktree root is also accepted.
    ///
    /// Returns `Ok(None)` if the resolved path has no dirty or staged files.
    /// Clean tracked files are not included in this aggregate
    /// change set. Files ignored by the VCS are also omitted because this
    /// crate is intended for `--allow-*` style checks, which treat them the
    /// same as clean files.
    ///
    /// Paths in the returned changes are relative to [`Self::worktree`].
    ///
    /// `wt_path` is interpreted relative to [`Self::worktree`]. If you have an
    /// absolute path or a path relative to the current working directory, use
    /// [`Self::resolve_path`] first. Symlinks are followed.
    ///
    /// If the resolved path does not exist in the worktree, this method may
    /// still return changes when the path refers to paths known to the VCS,
    /// such as a deleted tracked file path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - `wt_path` does not resolve to a path within [`Self::worktree`]
    /// - `wt_path` does not exist in the worktree and does not refer to a path
    ///   known to the VCS
    /// - `wt_path` could not be resolved to a canonical path for any other reason
    /// - the backend fails to query changes for `wt_path` for any other reason
    #[inline]
    pub fn path_changes<P>(&self, wt_path: P) -> Result<Option<RepositoryChanges>, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        self.inner.path_changes(wt_path.as_ref())
    }

    /// Returns the dirty and/or staged change for the worktree-relative file
    /// path `wt_path` within the repository, if any.
    ///
    /// Returns `Ok(None)` if the resolved file path is clean or ignored by the
    /// VCS.
    ///
    /// If this method returns `Ok(Some(change))`, the returned [`FileChange`]
    /// describes the resolved file path.
    ///
    /// `wt_path` is interpreted relative to [`Self::worktree`]. If you have an
    /// absolute path or a path relative to the current working directory, use
    /// [`Self::resolve_path`] first. Symlinks are followed.
    ///
    /// If the resolved path exists in the worktree, it must be a file.
    /// If it does not exist in the worktree, this method may still return a
    /// change when the path refers to a tracked file path known to the VCS,
    /// such as a deletion change. [`FileChange::wt_path`] may differ from the
    /// path passed to this method when the input reaches the same file path
    /// through symlinks or other equivalent non-canonical forms.
    ///
    /// A file may be both staged and dirty at the same time if it has staged
    /// changes and additional unstaged tracked changes.
    ///
    /// This operation is intended for file paths. It does not perform rename
    /// detection.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - `wt_path` does not resolve to a path within [`Self::worktree`]
    /// - the resolved path exists in the worktree but does not resolve to a
    ///   file
    /// - `wt_path` does not exist in the worktree and does not refer to a tracked
    ///   path known to the VCS
    /// - `wt_path` could not be resolved to a canonical path for any other reason
    /// - the backend fails to query file changes for any other reason
    #[inline]
    pub fn file_change<P>(&self, wt_path: P) -> Result<Option<FileChange>, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        self.inner.file_change(wt_path.as_ref())
    }
}

/// A non-empty set of file changes.
///
/// Values of this type are returned by [`Repository::repository_changes`] and
/// [`Repository::path_changes`].
///
/// This type contains only dirty and/or staged files. Dirty files have
/// unstaged modifications or are untracked. Clean tracked files are not
/// included. Files ignored by the VCS are also omitted
/// because this crate is intended for `--allow-*` style checks, which treat
/// them the same as clean files.
///
/// [`Repository::repository_changes`] and [`Repository::path_changes`] return
/// `None` instead of an empty change set.
///
/// File changes are ordered by ascending worktree-relative path. Entries may
/// include tracked paths that are no longer present in the worktree.
#[derive(Debug, Clone)]
pub struct RepositoryChanges {
    files: Vec<FileChange>,
    num_dirty_files: usize,
    num_staged_files: usize,
}

impl RepositoryChanges {
    #[cfg(any(test, vcs_backend_enabled))]
    pub(crate) fn new<I>(files: I) -> Option<Self>
    where
        I: IntoIterator<Item = FileChange>,
    {
        let mut files = files.into_iter().collect::<Vec<_>>();
        files.sort_by(|a, b| a.wt_path().cmp(b.wt_path()));
        assert!(
            files
                .array_windows()
                .all(|[a, b]| a.wt_path() != b.wt_path()),
            "repository change entries must be unique by worktree-relative path",
        );

        if files.is_empty() {
            return None;
        }

        let mut num_dirty_files = 0;
        let mut num_staged_files = 0;

        for file in &files {
            num_dirty_files += usize::from(file.is_dirty());
            num_staged_files += usize::from(file.is_staged());
        }

        Some(Self {
            files,
            num_dirty_files,
            num_staged_files,
        })
    }

    /// Returns an iterator over all file changes in this change set.
    ///
    /// Files are yielded in ascending worktree-relative path order.
    #[inline]
    #[must_use]
    pub fn files(&self) -> Files<'_> {
        Files {
            iter: self.files.iter(),
        }
    }

    /// Returns an iterator over dirty files in this change set.
    ///
    /// A dirty file has unstaged modifications or is untracked.
    /// Staged-only changes are not considered dirty.
    /// Files are yielded in ascending worktree-relative path order.
    #[inline]
    #[must_use]
    pub fn dirty_files(&self) -> DirtyFiles<'_> {
        DirtyFiles {
            iter: self.files.iter(),
            len: self.num_dirty_files,
        }
    }

    /// Returns an iterator over files with staged changes in this change set.
    ///
    /// Files are yielded in ascending worktree-relative path order.
    #[inline]
    #[must_use]
    pub fn staged_files(&self) -> StagedFiles<'_> {
        StagedFiles {
            iter: self.files.iter(),
            len: self.num_staged_files,
        }
    }

    /// Returns whether this change set contains any dirty files.
    ///
    /// A dirty file has unstaged modifications or is untracked.
    /// Staged-only changes are not considered dirty.
    #[inline]
    #[must_use]
    pub fn has_dirty_files(&self) -> bool {
        self.num_dirty_files > 0
    }

    /// Returns whether this change set contains any files with staged changes.
    #[inline]
    #[must_use]
    pub fn has_staged_files(&self) -> bool {
        self.num_staged_files > 0
    }
}

/// A file change within a repository.
///
/// Values of this type are returned by [`Repository::file_change`] and yielded
/// by iterators over [`RepositoryChanges`] returned by
/// [`Repository::repository_changes`] and [`Repository::path_changes`].
///
/// Instances of this type always represent a dirty file, a staged file, or
/// both. Dirty files have unstaged modifications or are untracked. Clean
/// tracked files and files ignored by the VCS are not represented;
/// [`Repository::file_change`] returns `None` for those cases.
///
/// The stored path is the worktree-relative path associated with this change
/// in the VCS. It may refer to a tracked path that is no longer present in the
/// worktree.
///
/// More than one predicate may return `true` for the same file. For example,
/// a file may have staged changes and additional unstaged tracked
/// modifications.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub(crate) wt_path: PathBuf,
    pub(crate) dirty: bool,
    pub(crate) staged: bool,
}

impl FileChange {
    /// Returns the worktree-relative path associated with this file change in
    /// the VCS.
    ///
    /// This may refer to a tracked path that is no longer present in the
    /// worktree.
    ///
    /// When this change is returned by [`Repository::file_change`], the
    /// returned path may differ from the path passed to that method when the
    /// input reaches the same path through symlinks or other equivalent
    /// non-canonical forms.
    #[inline]
    #[must_use]
    pub fn wt_path(&self) -> &Path {
        &self.wt_path
    }

    /// Returns whether the file is dirty.
    ///
    /// A dirty file has unstaged modifications or is untracked.
    /// Staged-only changes are not considered dirty.
    #[inline]
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns whether the file has staged changes in the index.
    #[inline]
    #[must_use]
    pub fn is_staged(&self) -> bool {
        self.staged
    }
}

/// An iterator over all file changes in a [`RepositoryChanges`].
///
/// This struct is created by the [`RepositoryChanges::files`] method.
/// Files are yielded in ascending worktree-relative path order.
#[derive(Debug, Clone)]
pub struct Files<'a> {
    iter: slice::Iter<'a, FileChange>,
}

impl<'a> Iterator for Files<'a> {
    type Item = &'a FileChange;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl DoubleEndedIterator for Files<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl ExactSizeIterator for Files<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

/// An iterator over dirty files in a [`RepositoryChanges`].
///
/// This struct is created by the [`RepositoryChanges::dirty_files`] method.
/// Files are yielded in ascending worktree-relative path order.
#[derive(Debug, Clone)]
pub struct DirtyFiles<'a> {
    iter: slice::Iter<'a, FileChange>,
    len: usize,
}

impl<'a> Iterator for DirtyFiles<'a> {
    type Item = &'a FileChange;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let file = self.iter.find(|file| file.is_dirty())?;
        self.len -= 1;
        Some(file)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for DirtyFiles<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let file = self.iter.rfind(|file| file.is_dirty())?;
        self.len -= 1;
        Some(file)
    }
}

impl ExactSizeIterator for DirtyFiles<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

/// An iterator over files with staged changes in a [`RepositoryChanges`].
///
/// This struct is created by the [`RepositoryChanges::staged_files`] method.
/// Files are yielded in ascending worktree-relative path order.
#[derive(Debug, Clone)]
pub struct StagedFiles<'a> {
    iter: slice::Iter<'a, FileChange>,
    len: usize,
}

impl<'a> Iterator for StagedFiles<'a> {
    type Item = &'a FileChange;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let file = self.iter.find(|file| file.is_staged())?;
        self.len -= 1;
        Some(file)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for StagedFiles<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let file = self.iter.rfind(|file| file.is_staged())?;
        self.len -= 1;
        Some(file)
    }
}

impl ExactSizeIterator for StagedFiles<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
