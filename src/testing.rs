#![allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use assert_fs::{
    TempDir,
    fixture::{ChildPath, PathChild},
};

use crate::{FileStatus, RepositoryStatus};

#[must_use]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AssertRepositoryStatus {
    modified: BTreeSet<PathBuf>,
    staged: BTreeSet<PathBuf>,
    untracked: BTreeSet<PathBuf>,
}

impl From<RepositoryStatus> for AssertRepositoryStatus {
    fn from(status: RepositoryStatus) -> Self {
        let RepositoryStatus {
            modified,
            staged,
            untracked,
        } = status;
        Self {
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertRepositoryStatus {
    pub(crate) fn modified<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.modified.extend(paths.into_iter().map(Into::into));
        self
    }

    pub(crate) fn staged<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.staged.extend(paths.into_iter().map(Into::into));
        self
    }

    pub(crate) fn untracked<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.untracked.extend(paths.into_iter().map(Into::into));
        self
    }

    #[track_caller]
    pub(crate) fn assert(self, actual: RepositoryStatus) {
        assert_eq!(Self::from(actual), self);
    }
}

#[must_use]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AssertFileStatus {
    modified: bool,
    staged: bool,
    untracked: bool,
}

impl From<FileStatus> for AssertFileStatus {
    fn from(status: FileStatus) -> Self {
        let FileStatus {
            modified,
            staged,
            untracked,
        } = status;
        Self {
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertFileStatus {
    pub(crate) fn modified(mut self) -> Self {
        self.modified = true;
        self
    }

    pub(crate) fn staged(mut self) -> Self {
        self.staged = true;
        self
    }

    pub(crate) fn untracked(mut self) -> Self {
        self.untracked = true;
        self
    }

    #[track_caller]
    pub(crate) fn assert(self, actual: FileStatus) {
        assert_eq!(Self::from(actual), self);
    }
}

pub(crate) type DropGuard = Box<dyn FnOnce(&mut PathInTempDir)>;

#[must_use]
pub(crate) struct PathInTempDir {
    _tempdir: TempDir,
    path: PathBuf,
    drop_guard: Option<DropGuard>,
}

impl Drop for PathInTempDir {
    fn drop(&mut self) {
        if let Some(drop_guard) = self.drop_guard.take() {
            drop_guard(self);
        }
    }
}

impl PathInTempDir {
    pub(crate) fn new() -> Self {
        let tempdir = TempDir::new().unwrap();
        // Canonicalize the tempdir path because backends may return the repository
        // worktree in canonical form even when the input path is not, such as macOS
        // `/var` vs `/private/var`. Use `dunce` to avoid introducing Windows
        // verbatim paths in test fixtures.
        let path = dunce::canonicalize(&tempdir).unwrap();
        PathInTempDir {
            _tempdir: tempdir,
            path,
            drop_guard: None,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn set_path<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        self.path = path.into();
    }

    pub(crate) fn set_drop_guard<F>(&mut self, drop_guard: F)
    where
        F: FnOnce(&mut PathInTempDir) + 'static,
    {
        self.drop_guard = Some(Box::new(drop_guard));
    }
}

impl AsRef<Path> for PathInTempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl PathChild for PathInTempDir {
    fn child<P>(&self, path: P) -> ChildPath
    where
        P: AsRef<Path>,
    {
        ChildPath::new(self.path.join(path))
    }
}
