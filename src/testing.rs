#![allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use assert_fs::{
    TempDir,
    fixture::{ChildPath, PathChild},
};

use crate::repository::{FileChange, RepositoryChanges};

#[must_use]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AssertRepositoryChanges {
    files: BTreeMap<PathBuf, AssertFileChange>,
    modified: BTreeSet<PathBuf>,
    staged: BTreeSet<PathBuf>,
    untracked: BTreeSet<PathBuf>,
}

impl From<RepositoryChanges> for AssertRepositoryChanges {
    fn from(changes: RepositoryChanges) -> Self {
        fn collect_path<'a, I>(iter: I) -> BTreeSet<PathBuf>
        where
            I: IntoIterator<Item = &'a FileChange>,
        {
            iter.into_iter().map(|file| file.path.clone()).collect()
        }

        let files = changes
            .files()
            .map(|file| (file.path().to_owned(), AssertFileChange::from(file.clone())))
            .collect();

        let paths = collect_path(changes.files());
        let modified = collect_path(changes.modified_files());
        assert!(modified.is_subset(&paths));
        let staged = collect_path(changes.staged_files());
        assert!(staged.is_subset(&paths));
        let untracked = collect_path(changes.untracked_files());
        assert!(untracked.is_subset(&paths));

        Self {
            files,
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertRepositoryChanges {
    pub(crate) fn modified<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for path in paths {
            let path = path.into();
            let file = self
                .files
                .entry(path.clone())
                .or_insert_with(|| AssertFileChange::new(path.clone()));
            file.modified = true;
            self.modified.insert(path);
        }
        self
    }

    pub(crate) fn staged<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for path in paths {
            let path = path.into();
            let file = self
                .files
                .entry(path.clone())
                .or_insert_with(|| AssertFileChange::new(path.clone()));
            file.staged = true;
            self.staged.insert(path);
        }
        self
    }

    pub(crate) fn untracked<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for path in paths {
            let path = path.into();
            let file = self
                .files
                .entry(path.clone())
                .or_insert_with(|| AssertFileChange::new(path.clone()));
            file.untracked = true;
            self.untracked.insert(path);
        }
        self
    }

    #[track_caller]
    pub(crate) fn assert(self, actual: RepositoryChanges) {
        let actual = Self::from(actual);
        assert_eq!(actual, self);
    }
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AssertFileChange {
    path: PathBuf,
    modified: bool,
    staged: bool,
    untracked: bool,
}

impl From<FileChange> for AssertFileChange {
    fn from(change: FileChange) -> Self {
        let FileChange {
            path,
            modified,
            staged,
            untracked,
        } = change;
        Self {
            path,
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertFileChange {
    pub(crate) fn new<P>(path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            path: path.into(),
            modified: false,
            staged: false,
            untracked: false,
        }
    }

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
    pub(crate) fn assert(self, actual: FileChange) {
        let actual = Self::from(actual);
        assert_eq!(actual, self);
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
