use std::path::{Path, PathBuf};

use crate::{Repository, VcsStatusError, repository::RepositoryChanges};

#[cfg(test)]
mod tests;

/// Options for `--allow-*` style safety checks before modifying files.
///
/// This type matches the semantics of `cargo fix`:
///
/// - `allow_no_vcs` allows modifying the path even when no supported VCS
///   repository is found
/// - `allow_dirty` allows modifying the path even when it is dirty or has
///   staged changes
/// - `allow_staged` allows modifying the path even when it has staged changes,
///   but still rejects dirty files
///
/// By default, checks are scoped to the queried path. Use
/// [`Self::check_entire_repository`] to check the containing repository as a
/// whole instead.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
///
/// use vcs_status::{AllowOptions, CheckResult};
///
/// let result = AllowOptions::new()
///     .allow_staged(true)
///     .check_safe_to_modify(Path::new("."))?;
///
/// match result {
///     CheckResult::Allowed => {}
///     CheckResult::BlockedByNoVcs => {
///         eprintln!("The target path is not in a VCS repository.");
///     }
///     CheckResult::BlockedByDirty { dirty_files, .. } => {
///         eprintln!("Dirty files:");
///         for path in dirty_files {
///             eprintln!("* {}", path.display());
///         }
///     }
///     CheckResult::BlockedByStaged { staged_files, .. } => {
///         eprintln!("Staged files:");
///         for path in staged_files {
///             eprintln!("* {}", path.display());
///         }
///     }
/// }
/// # Ok::<(), vcs_status::VcsStatusError>(())
/// ```
#[expect(
    missing_copy_implementations,
    reason = "Copy is intentionally not part of the API contract"
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "This struct represents independent `--allow-*` and scope configuration flags whose combinations are meaningful, not a state machine"
)]
#[derive(Debug, Clone)]
pub struct AllowOptions {
    allow_no_vcs: bool,
    allow_dirty: bool,
    allow_staged: bool,
    check_entire_repository: bool,
}

impl Default for AllowOptions {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl AllowOptions {
    /// Creates an `AllowOptions` value with all `--allow-*` options disabled.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allow_no_vcs: false,
            allow_dirty: false,
            allow_staged: false,
            check_entire_repository: false,
        }
    }

    /// Sets whether to allow modifying the queried path when no supported VCS
    /// repository is found.
    #[inline]
    #[must_use]
    pub const fn allow_no_vcs(mut self, enabled: bool) -> Self {
        self.allow_no_vcs = enabled;
        self
    }

    /// Sets whether to allow dirty files and staged changes.
    ///
    /// When enabled, this also allows staged changes, matching `cargo fix`.
    #[inline]
    #[must_use]
    pub const fn allow_dirty(mut self, enabled: bool) -> Self {
        self.allow_dirty = enabled;
        self
    }

    /// Sets whether to allow staged changes.
    ///
    /// Dirty files are still rejected when this option is enabled.
    #[inline]
    #[must_use]
    pub const fn allow_staged(mut self, enabled: bool) -> Self {
        self.allow_staged = enabled;
        self
    }

    /// Sets whether checks should cover the entire containing repository
    /// rather than only the queried path.
    #[inline]
    #[must_use]
    pub const fn check_entire_repository(mut self, enabled: bool) -> Self {
        self.check_entire_repository = enabled;
        self
    }

    fn find_changes<R>(
        &self,
        repo: &R,
        path: &Path,
    ) -> Result<Option<RepositoryChanges>, VcsStatusError>
    where
        R: AllowOptionsRepository,
    {
        if self.check_entire_repository {
            repo.repository_changes()
        } else {
            repo.path_changes(path)
        }
    }

    /// Checks whether it is safe to modify `path` under the current
    /// `--allow-*` settings.
    ///
    /// This discovers the containing repository for `path`, unless
    /// [`Self::allow_no_vcs`] is enabled.
    ///
    /// When [`Self::check_entire_repository`] is disabled, the check is scoped
    /// to `path`. When enabled, the entire containing repository is checked.
    ///
    /// # Errors
    ///
    /// Returns an error if repository discovery fails, if `path` cannot be
    /// resolved for change queries, or if the backend fails to query the
    /// relevant changes.
    #[inline]
    pub fn check_safe_to_modify<P>(&self, path: P) -> Result<CheckResult, VcsStatusError>
    where
        P: AsRef<Path>,
    {
        self.check_safe_to_modify_with_backend(path, &RealBackend)
    }

    fn check_safe_to_modify_with_backend<P, B>(
        &self,
        path: P,
        backend: &B,
    ) -> Result<CheckResult, VcsStatusError>
    where
        P: AsRef<Path>,
        B: AllowOptionsBackend,
    {
        // Match `cargo fix` exactly:
        // - `--allow-no-vcs` allows modifying the path even if a VCS was not
        //   detected.
        // - `--allow-dirty` allows modifying the path even if it is dirty or
        //   has staged changes.
        // - `--allow-staged` allows modifying the path even if it has staged
        //   changes.

        let path = path.as_ref();

        if self.allow_no_vcs {
            return Ok(CheckResult::Allowed);
        }

        let Some(repo) = backend.discover(path)? else {
            return Ok(CheckResult::BlockedByNoVcs);
        };

        if self.allow_dirty {
            return Ok(CheckResult::Allowed);
        }

        let Some(changes) = self.find_changes(&repo, path)? else {
            return Ok(CheckResult::Allowed);
        };

        let dirty_files = changes
            .files()
            .filter(|f| f.is_dirty())
            .map(|f| f.path().to_owned())
            .collect::<Vec<_>>();

        if self.allow_staged {
            if !dirty_files.is_empty() {
                return Ok(CheckResult::BlockedByDirty {
                    worktree: repo.worktree().to_owned(),
                    dirty_files,
                    staged_files: vec![],
                });
            }
            return Ok(CheckResult::Allowed);
        }

        let staged_files = changes
            .files()
            .filter(|f| f.is_staged())
            .map(|f| f.path().to_owned())
            .collect::<Vec<_>>();

        if dirty_files.is_empty() {
            return Ok(CheckResult::BlockedByStaged {
                worktree: repo.worktree().to_owned(),
                staged_files,
            });
        }

        Ok(CheckResult::BlockedByDirty {
            worktree: repo.worktree().to_owned(),
            dirty_files,
            staged_files,
        })
    }
}

trait AllowOptionsBackend {
    type Repo: AllowOptionsRepository;

    fn discover(&self, path: &Path) -> Result<Option<Self::Repo>, VcsStatusError>;
}

trait AllowOptionsRepository {
    fn worktree(&self) -> &Path;
    fn path_changes(&self, path: &Path) -> Result<Option<RepositoryChanges>, VcsStatusError>;
    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, VcsStatusError>;
}

struct RealBackend;

impl AllowOptionsBackend for RealBackend {
    type Repo = Repository;

    fn discover(&self, path: &Path) -> Result<Option<Self::Repo>, VcsStatusError> {
        Repository::discover(path)
    }
}

impl AllowOptionsRepository for Repository {
    fn worktree(&self) -> &Path {
        self.worktree()
    }
    fn path_changes(&self, path: &Path) -> Result<Option<RepositoryChanges>, VcsStatusError> {
        self.path_changes(path)
    }
    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, VcsStatusError> {
        self.repository_changes()
    }
}

/// The result of checking whether a path may be safely modified.
#[expect(
    clippy::exhaustive_enums,
    reason = "Callers should exhaustively match the current outcomes; adding a new variant is an intentional breaking API change"
)]
#[derive(Debug)]
pub enum CheckResult {
    /// The operation is allowed.
    Allowed,
    /// The operation was blocked because no supported VCS repository was found.
    BlockedByNoVcs,
    /// The operation was blocked by dirty files.
    ///
    /// `staged_files` is non-empty only when staged changes also block the
    /// operation.
    BlockedByDirty {
        /// The root directory of the repository worktree.
        worktree: PathBuf,
        /// Repository worktree-relative paths of files that block the
        /// operation because they are dirty.
        dirty_files: Vec<PathBuf>,
        /// Repository worktree-relative paths of staged files that also block
        /// the operation.
        staged_files: Vec<PathBuf>,
    },
    /// The operation was blocked only by staged changes.
    BlockedByStaged {
        /// The root directory of the repository worktree.
        worktree: PathBuf,
        /// Repository worktree-relative paths of staged files that block the
        /// operation.
        staged_files: Vec<PathBuf>,
    },
}
