use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use gix::{bstr::BString, status::index_worktree::iter::Summary};
use snafu::{IntoError as _, ResultExt as _, Snafu};

use super::VcsRepository;
use crate::{
    ModifyGuardError,
    error::{self},
    repository::{FileChange, RepositoryChanges},
    util::{self, NormalizedPath},
    vcs::VcsBackend,
};

pub(super) const BACKEND: GixBackend = GixBackend;

#[derive(Debug)]
pub(super) struct GixBackend;

/// Errors returned by `gix` backend operations.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum GixBackendError {
    /// Searching for a Git repository failed.
    #[snafu(display("failed while searching for a git repository at or above path: {}", path.display()))]
    Discover {
        /// The path that was being searched for a Git repository.
        path: PathBuf,
        /// The underlying error from `gix`.
        source: gix::discover::Error,
    },
    /// Opening a Git repository failed.
    #[snafu(display("failed to open git repository at path: {}", path.display()))]
    Open {
        /// The path that was being opened as a Git repository.
        path: PathBuf,
        /// The underlying error from `gix`.
        source: gix::open::Error,
    },
    /// Querying the status of a Git repository failed.
    #[snafu(display("failed to query git repository status for worktree: {}", worktree.display()))]
    Status {
        /// The worktree of the Git repository.
        worktree: PathBuf,
        /// The underlying error from `gix`.
        source: gix::status::Error,
    },
    /// Converting the status of a Git repository into an iterator failed.
    #[snafu(display("failed to convert git repository status into iterator for worktree: {}", worktree.display()))]
    StatusIntoIter {
        /// The worktree of the Git repository.
        worktree: PathBuf,
        /// The underlying error from `gix`.
        source: gix::status::into_iter::Error,
    },
    /// Iterating over the status of a Git repository failed.
    #[snafu(display("failed to iterate git repository status for worktree: {}", worktree.display()))]
    IterateStatus {
        /// The worktree of the Git repository.
        worktree: PathBuf,
        /// The underlying error from `gix`.
        source: gix::status::iter::Error,
    },
    /// A file query was ambiguous and matched an unexpected path.
    #[snafu(display(
        "git file query for {} was ambiguous",
        query.display()
    ))]
    AmbiguousFilePath {
        /// The worktree-relative path requested by the file query.
        query: PathBuf,
    },
}

impl From<GixBackendError> for ModifyGuardError {
    #[inline]
    fn from(source: GixBackendError) -> Self {
        Self::Backend {
            source: source.into(),
        }
    }
}

impl VcsBackend for GixBackend {
    fn discover(
        &self,
        mut path: &Path,
    ) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
        util::ensure_path_exists(path)?;
        #[expect(
            clippy::unwrap_used,
            reason = "path is guaranteed to have a parent because it exists and is a file"
        )]
        if path.is_file() {
            path = path.parent().unwrap();
        }

        let repo = match gix::discover(path) {
            Ok(repo) => repo,
            Err(gix::discover::Error::Discover(
                gix::discover::upwards::Error::NoGitRepository { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinCeiling { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinFs { .. },
            )) => return Ok(None),
            Err(source) => return Err(DiscoverSnafu { path }.into_error(source).into()),
        };
        let Some(worktree) = repo.workdir().map(Path::to_owned) else {
            return Err(error::RepositoryWithoutWorktreeSnafu {
                path: repo.git_dir(),
            }
            .build());
        };
        Ok(Some(Box::new(GixRepository { repo, worktree })))
    }

    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
        util::ensure_path_is_directory(path)?;

        let repo = match gix::open(path) {
            Ok(repo) => repo,
            Err(gix::open::Error::NotARepository { .. }) => return Ok(None),
            Err(source) => return Err(OpenSnafu { path }.into_error(source).into()),
        };
        let Some(worktree) = repo.workdir().map(Path::to_owned) else {
            return Err(error::RepositoryWithoutWorktreeSnafu { path }.build());
        };
        Ok(Some(Box::new(GixRepository { repo, worktree })))
    }
}

struct GixRepository {
    repo: gix::Repository,
    worktree: PathBuf,
}

impl fmt::Debug for GixRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GixRepository")
            .field("repo", &"<gix::Repository>")
            .field("worktree", &self.worktree)
            .finish()
    }
}

impl VcsRepository for GixRepository {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        Ok(RepositoryChanges::new(self.collect_changes(None)?))
    }

    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        let wt_path = util::normalize_worktree_path(&self.worktree, wt_path)?;
        if wt_path.is_empty() {
            return self.repository_changes();
        }

        let changes = self.collect_changes(Some(&wt_path))?;

        if changes.is_empty() {
            if matches!(wt_path, NormalizedPath::Missing(_)) {
                return Err(error::PathNotFoundSnafu {
                    path: wt_path.as_path(),
                }
                .build());
            }
            return Ok(None);
        }

        Ok(RepositoryChanges::new(changes))
    }

    fn file_change(&self, wt_path: &Path) -> Result<Option<FileChange>, ModifyGuardError> {
        let wt_path = util::normalize_worktree_path(&self.worktree, wt_path)?;
        match &wt_path {
            NormalizedPath::Existing(wt_path) => {
                let fs_path = self.worktree.join(wt_path);
                util::ensure_path_is_file(&fs_path)?;
            }
            NormalizedPath::Missing(_) => {}
        }

        let changes = self.collect_changes(Some(&wt_path))?;

        match changes.as_slice() {
            [] => match wt_path {
                NormalizedPath::Existing(_) => Ok(None),
                NormalizedPath::Missing(_) => {
                    Err(error::PathNotFoundSnafu { path: wt_path }.build())
                }
            },
            [change] if change.wt_path() == wt_path.as_path() => Ok(Some(change.clone())),
            [..] => Err(AmbiguousFilePathSnafu { query: wt_path }.build().into()),
        }
    }
}

impl GixRepository {
    fn collect_changes(
        &self,
        wt_path: Option<&NormalizedPath>,
    ) -> Result<Vec<FileChange>, ModifyGuardError> {
        let worktree = &self.worktree;
        let status_platform = self
            .repo
            .status(gix::progress::Discard)
            .context(StatusSnafu { worktree })?
            .untracked_files(gix::status::UntrackedFiles::Files)
            .index_worktree_rewrites(None)
            .tree_index_track_renames(gix::status::tree_index::TrackRenames::Disabled);
        let patterns = wt_path.map(|wt_path| literal_pathspec(wt_path.as_path()));
        let status_iter = status_platform
            .into_iter(patterns)
            .context(StatusIntoIterSnafu { worktree })?;

        let mut changes = BTreeMap::<PathBuf, StatusFlags>::new();
        for item in status_iter {
            let item = item.context(IterateStatusSnafu { worktree })?;
            let wt_path = item.location();
            let status = match &item {
                gix::status::Item::TreeIndex(_change) => StatusFlags::STAGED,
                gix::status::Item::IndexWorktree(item) => {
                    let Some(summary) = item.summary() else {
                        continue;
                    };
                    match summary {
                        Summary::Removed
                        | Summary::Added
                        | Summary::Modified
                        | Summary::TypeChange
                        | Summary::Renamed
                        | Summary::Copied
                        | Summary::IntentToAdd => StatusFlags::DIRTY,
                        Summary::Conflict => StatusFlags::DIRTY_AND_STAGED,
                    }
                }
            };
            changes
                .entry(gix::path::from_bstring(wt_path))
                .or_default()
                .merge(status);
        }

        Ok(changes
            .into_iter()
            .filter_map(|(wt_path, status)| status.build(wt_path))
            .collect())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct StatusFlags {
    dirty: bool,
    staged: bool,
}

impl StatusFlags {
    const DIRTY: Self = Self {
        dirty: true,
        staged: false,
    };
    const STAGED: Self = Self {
        dirty: false,
        staged: true,
    };
    const DIRTY_AND_STAGED: Self = Self {
        dirty: true,
        staged: true,
    };

    fn merge(&mut self, other: Self) {
        self.dirty |= other.dirty;
        self.staged |= other.staged;
    }

    fn build(self, wt_path: PathBuf) -> Option<FileChange> {
        let Self { dirty, staged } = self;
        if !dirty && !staged {
            return None;
        }
        Some(FileChange {
            wt_path,
            dirty,
            staged,
        })
    }
}

fn literal_pathspec(path: &Path) -> BString {
    let mut pattern = BString::from(":(top,literal)");
    pattern.extend_from_slice(&gix::path::into_bstr(path));
    pattern
}
