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

pub(crate) fn modified_file<P>(wt_path: P) -> FileChange
where
    P: Into<PathBuf>,
{
    FileChange {
        wt_path: wt_path.into(),
        modified: true,
        staged: false,
        untracked: false,
    }
}

pub(crate) fn staged_file<P>(wt_path: P) -> FileChange
where
    P: Into<PathBuf>,
{
    FileChange {
        wt_path: wt_path.into(),
        modified: false,
        staged: true,
        untracked: false,
    }
}

pub(crate) fn modified_and_staged_file<P>(wt_path: P) -> FileChange
where
    P: Into<PathBuf>,
{
    FileChange {
        wt_path: wt_path.into(),
        modified: true,
        staged: true,
        untracked: false,
    }
}

pub(crate) fn untracked_file<P>(wt_path: P) -> FileChange
where
    P: Into<PathBuf>,
{
    FileChange {
        wt_path: wt_path.into(),
        modified: false,
        staged: false,
        untracked: true,
    }
}

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
        fn collect_wt_paths<'a, I>(iter: I) -> BTreeSet<PathBuf>
        where
            I: IntoIterator<Item = &'a FileChange>,
        {
            iter.into_iter().map(|file| file.wt_path.clone()).collect()
        }

        let files = changes
            .files()
            .map(|file| {
                (
                    file.wt_path().to_owned(),
                    AssertFileChange::from(file.clone()),
                )
            })
            .collect();

        let wt_paths = collect_wt_paths(changes.files());
        let modified = collect_wt_paths(changes.modified_files());
        assert!(modified.is_subset(&wt_paths));
        let staged = collect_wt_paths(changes.staged_files());
        assert!(staged.is_subset(&wt_paths));
        let untracked = collect_wt_paths(changes.untracked_files());
        assert!(untracked.is_subset(&wt_paths));

        Self {
            files,
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertRepositoryChanges {
    pub(crate) fn modified<I, P>(mut self, wt_paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for wt_path in wt_paths {
            let wt_path = wt_path.into();
            let file = self
                .files
                .entry(wt_path.clone())
                .or_insert_with(|| AssertFileChange::new(wt_path.clone()));
            file.modified = true;
            self.modified.insert(wt_path);
        }
        self
    }

    pub(crate) fn staged<I, P>(mut self, wt_paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for wt_path in wt_paths {
            let wt_path = wt_path.into();
            let file = self
                .files
                .entry(wt_path.clone())
                .or_insert_with(|| AssertFileChange::new(wt_path.clone()));
            file.staged = true;
            self.staged.insert(wt_path);
        }
        self
    }

    pub(crate) fn untracked<I, P>(mut self, wt_paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        for wt_path in wt_paths {
            let wt_path = wt_path.into();
            let file = self
                .files
                .entry(wt_path.clone())
                .or_insert_with(|| AssertFileChange::new(wt_path.clone()));
            file.untracked = true;
            self.untracked.insert(wt_path);
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
    wt_path: PathBuf,
    modified: bool,
    staged: bool,
    untracked: bool,
}

impl From<FileChange> for AssertFileChange {
    fn from(change: FileChange) -> Self {
        let FileChange {
            wt_path,
            modified,
            staged,
            untracked,
        } = change;
        Self {
            wt_path,
            modified,
            staged,
            untracked,
        }
    }
}

impl AssertFileChange {
    pub(crate) fn new<P>(wt_path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            wt_path: wt_path.into(),
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
