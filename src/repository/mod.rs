//! Repository change query APIs.
//!
//! This module provides the [`Repository`] type for discovering and querying
//! VCS repositories, along with change result types such as
//! [`RepositoryChanges`] and [`FileChange`].
//!
//! Paths returned by change queries are relative to the repository worktree.
//!
//! Most users will start with [`Repository::discover`] or [`Repository::open`],
//! then query changes with [`Repository::repository_changes`] or
//! [`Repository::file_change`].

use std::{
    path::{Path, PathBuf},
    slice,
};

use crate::{
    VcsStatusError,
    vcs::{self, VcsRepository},
};

#[cfg(test)]
mod tests;

/// A handle for querying changes in a VCS repository worktree.
///
/// This type is used to check whether a CLI tool may safely modify files,
/// such as for `--allow-*` style checks.
///
/// Repositories without a worktree, such as Git bare repositories, are not
/// represented by this type.
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
    /// Returns an error if:
    ///
    /// - `path` does not refer to a supported repository worktree
    /// - the backend fails to open it
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
        self.inner.worktree()
    }

    /// Returns the aggregate file changes in the repository worktree.
    ///
    /// Paths in the returned changes are relative to [`Self::workdir`].
    ///
    /// Returns `Ok(None)` if the repository has no modified, staged, or
    /// untracked files. Clean tracked files are not included in this aggregate
    /// change set. Files ignored by the VCS are also omitted because this
    /// crate is intended for `--allow-*` style checks, which treat them the
    /// same as clean files.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to query repository changes.
    #[inline]
    pub fn repository_changes(&self) -> Result<Option<RepositoryChanges>, VcsStatusError> {
        self.inner.repository_changes()
    }

    /// Returns the modified, staged, or untracked change for a single file
    /// `path` within the repository, if any.
    ///
    /// Returns `Ok(None)` if the resolved file is clean or ignored by the VCS.
    ///
    /// If this method returns `Ok(Some(change))`, the returned [`FileChange`]
    /// describes the resolved file.
    ///
    /// `path` must resolve to an existing file within [`Self::workdir`].
    /// Symlinks are followed, and the resolved file must also be within
    /// [`Self::workdir`]. [`FileChange::path`] may differ from the path passed
    /// to this method when the input reaches the same file through symlinks or
    /// other equivalent non-canonical forms.
    ///
    /// A file may be both staged and modified at the same time if it has
    /// staged changes and additional unstaged changes.
    ///
    /// This operation is intended for file paths. It does not perform rename
    /// detection.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - `path` does not resolve to a path within [`Self::workdir`]
    /// - `path` does not resolve to an existing file in the worktree
    /// - `path` could not be resolved to a canonical path for any other reason
    /// - the backend fails to query file changes for any other reason
    #[inline]
    pub fn file_change<P>(&self, path: P) -> Result<Option<FileChange>, VcsStatusError>
    where
        P: AsRef<Path>,
    {
        self.inner.file_change(path.as_ref())
    }
}

/// A non-empty set of file changes.
///
/// Values of this type are returned by [`Repository::repository_changes`].
///
/// This type contains only modified, staged, or untracked files. Clean
/// tracked files are not included. Files ignored by the VCS are also omitted
/// because this crate is intended for `--allow-*` style checks, which treat
/// them the same as clean files.
///
/// [`Repository::repository_changes`] returns `None` instead of an empty
/// change set.
///
/// File changes are ordered by ascending worktree-relative path. Entries may
/// include tracked paths that are no longer present in the worktree.
#[derive(Debug, Clone)]
pub struct RepositoryChanges {
    files: Vec<FileChange>,
    num_modified_files: usize,
    num_staged_files: usize,
    num_untracked_files: usize,
}

impl RepositoryChanges {
    #[cfg(any(test, vcs_backend_enabled))]
    pub(crate) fn new<I>(files: I) -> Option<Self>
    where
        I: IntoIterator<Item = FileChange>,
    {
        let mut files = files.into_iter().collect::<Vec<_>>();
        files.sort_by(|a, b| a.path().cmp(b.path()));
        assert!(
            files.array_windows().all(|[a, b]| a.path() != b.path()),
            "repository change entries must be unique by path",
        );

        if files.is_empty() {
            return None;
        }

        let mut num_modified_files = 0;
        let mut num_staged_files = 0;
        let mut num_untracked_files = 0;

        for file in &files {
            num_modified_files += usize::from(file.is_modified());
            num_staged_files += usize::from(file.is_staged());
            num_untracked_files += usize::from(file.is_untracked());
        }

        Some(Self {
            files,
            num_modified_files,
            num_staged_files,
            num_untracked_files,
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

    /// Returns an iterator over files with unstaged changes in this change
    /// set.
    ///
    /// Files are yielded in ascending worktree-relative path order.
    #[inline]
    #[must_use]
    pub fn modified_files(&self) -> ModifiedFiles<'_> {
        ModifiedFiles {
            iter: self.files.iter(),
            len: self.num_modified_files,
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

    /// Returns an iterator over untracked files in this change set.
    ///
    /// Files are yielded in ascending worktree-relative path order.
    #[inline]
    #[must_use]
    pub fn untracked_files(&self) -> UntrackedFiles<'_> {
        UntrackedFiles {
            iter: self.files.iter(),
            len: self.num_untracked_files,
        }
    }

    /// Returns whether this change set contains any files with unstaged
    /// changes.
    ///
    /// This does not include staged changes or untracked files.
    #[inline]
    #[must_use]
    pub fn has_modified_files(&self) -> bool {
        self.num_modified_files > 0
    }

    /// Returns whether this change set contains any files with staged changes.
    #[inline]
    #[must_use]
    pub fn has_staged_files(&self) -> bool {
        self.num_staged_files > 0
    }

    /// Returns whether this change set contains any untracked files.
    #[inline]
    #[must_use]
    pub fn has_untracked_files(&self) -> bool {
        self.num_untracked_files > 0
    }
}

/// A file change within a repository.
///
/// Values of this type are returned by [`Repository::file_change`] and yielded
/// by iterators over [`RepositoryChanges`].
///
/// Instances of this type always represent a modified, staged, or untracked
/// file, or a combination of those states. Clean tracked files and files
/// ignored by the VCS are not represented; [`Repository::file_change`]
/// returns `None` for those cases.
///
/// The stored path is the worktree-relative path associated with this change
/// in the VCS. When returned by [`Repository::repository_changes`], it may
/// refer to a tracked path that is no longer present in the worktree.
///
/// More than one predicate may return `true` for the same file. For example,
/// a file may have staged changes and additional unstaged modifications.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub(crate) path: PathBuf,
    pub(crate) modified: bool,
    pub(crate) staged: bool,
    pub(crate) untracked: bool,
}

impl FileChange {
    /// Returns the worktree-relative path associated with this file change in
    /// the VCS.
    ///
    /// When this change is returned by [`Repository::file_change`], the
    /// returned path may differ from the path passed to that method when the
    /// input reaches the same file through symlinks or other equivalent
    /// non-canonical forms.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether the file has unstaged changes.
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

/// An iterator over files with unstaged changes in a [`RepositoryChanges`].
///
/// This struct is created by the [`RepositoryChanges::modified_files`] method.
/// Files are yielded in ascending worktree-relative path order.
#[derive(Debug, Clone)]
pub struct ModifiedFiles<'a> {
    iter: slice::Iter<'a, FileChange>,
    len: usize,
}

impl<'a> Iterator for ModifiedFiles<'a> {
    type Item = &'a FileChange;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let file = self.iter.find(|file| file.is_modified())?;
        self.len -= 1;
        Some(file)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for ModifiedFiles<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let file = self.iter.rfind(|file| file.is_modified())?;
        self.len -= 1;
        Some(file)
    }
}

impl ExactSizeIterator for ModifiedFiles<'_> {
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

/// An iterator over untracked files in a [`RepositoryChanges`].
///
/// This struct is created by the [`RepositoryChanges::untracked_files`] method.
/// Files are yielded in ascending worktree-relative path order.
#[derive(Debug, Clone)]
pub struct UntrackedFiles<'a> {
    iter: slice::Iter<'a, FileChange>,
    len: usize,
}

impl<'a> Iterator for UntrackedFiles<'a> {
    type Item = &'a FileChange;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let file = self.iter.find(|file| file.is_untracked())?;
        self.len -= 1;
        Some(file)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for UntrackedFiles<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let file = self.iter.rfind(|file| file.is_untracked())?;
        self.len -= 1;
        Some(file)
    }
}

impl ExactSizeIterator for UntrackedFiles<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
