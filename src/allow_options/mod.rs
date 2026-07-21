use std::path::{Path, PathBuf};

use crate::{
    ModifyGuardError,
    repository::{Repository, RepositoryChanges},
};

#[cfg(test)]
mod tests;

/// Options for `--allow-*` style safety checks before modifying files.
///
/// This is the main entry point for most users of this crate.
///
/// This type matches the semantics of `cargo fix`:
///
/// - `allow_no_vcs` treats modification as safe even when no supported VCS
///   repository is found
/// - `allow_dirty` treats modification as safe even when the path is dirty or
///   has staged changes
/// - `allow_staged` treats modification as safe even when the path has staged
///   changes, but still considers dirty files unsafe
///
/// These options are not interpreted independently. Higher-precedence options
/// imply lower-precedence ones, matching `cargo fix`:
///
/// - `allow_no_vcs` skips repository discovery and repository state checks
///   entirely
/// - `allow_dirty` still requires repository discovery, but implies
///   `allow_staged` and skips dirty and staged change checks
/// - `allow_staged` still requires repository discovery and change queries,
///   but dirty files remain unsafe
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
/// use vcs_modify_guard::{AllowOptions, ModificationSafety, UnsafeModificationReason};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let safety = AllowOptions::new()
///     .allow_staged(true)
///     .check_safe_to_modify(Path::new("."))?;
///
/// match safety {
///     ModificationSafety::Safe => {}
///     ModificationSafety::Unsafe(reason) => match reason {
///         UnsafeModificationReason::NoVcs => {
///             eprintln!("The target path is not in a VCS repository.");
///             return Err("blocked by no VCS".into());
///         }
///         UnsafeModificationReason::Dirty {
///             dirty_files,
///             staged_files,
///             ..
///         } => {
///             eprintln!("Dirty files:");
///             for wt_path in dirty_files {
///                 eprintln!("* {}", wt_path.display());
///             }
///             for wt_path in staged_files {
///                 eprintln!("* {} (staged)", wt_path.display());
///             }
///             return Err("blocked by dirty files".into());
///         }
///         UnsafeModificationReason::Staged { staged_files, .. } => {
///             eprintln!("Staged files:");
///             for wt_path in staged_files {
///                 eprintln!("* {}", wt_path.display());
///             }
///             return Err("blocked by staged changes".into());
///         }
///     },
/// }
/// # Ok(())
/// # }
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

    /// Sets whether to use `cargo fix`-style `--allow-no-vcs` behavior.
    ///
    /// When enabled, this skips repository discovery and repository state
    /// checks entirely, matching `cargo fix`. In other words, this does
    /// more than only relax the "no repository found" case.
    #[inline]
    #[must_use]
    pub const fn allow_no_vcs(mut self, enabled: bool) -> Self {
        self.allow_no_vcs = enabled;
        self
    }

    /// Sets whether to use `cargo fix`-style `--allow-dirty` behavior.
    ///
    /// When enabled, this still requires repository discovery unless
    /// [`Self::allow_no_vcs`] is enabled, but it treats both dirty files and
    /// staged changes as safe. This also implies [`Self::allow_staged`].
    #[inline]
    #[must_use]
    pub const fn allow_dirty(mut self, enabled: bool) -> Self {
        self.allow_dirty = enabled;
        self
    }

    /// Sets whether to use `cargo fix`-style `--allow-staged` behavior.
    ///
    /// When enabled, this still requires repository discovery and change
    /// queries unless [`Self::allow_no_vcs`] is enabled. Dirty files are still
    /// considered unsafe.
    #[inline]
    #[must_use]
    pub const fn allow_staged(mut self, enabled: bool) -> Self {
        self.allow_staged = enabled;
        self
    }

    /// Sets whether the safety check should cover the entire containing
    /// repository rather than only the queried path.
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
    ) -> Result<Option<RepositoryChanges>, ModifyGuardError>
    where
        R: AllowOptionsRepository,
    {
        if self.check_entire_repository {
            repo.repository_changes()
        } else {
            let wt_path = repo.resolve_path(path)?;
            repo.path_changes(&wt_path)
        }
    }

    /// Checks whether modification of `path` is considered safe under the
    /// current `--allow-*` settings.
    ///
    /// Flag handling matches `cargo fix`:
    ///
    /// - [`Self::allow_no_vcs`] returns [`ModificationSafety::Safe`] and skips
    ///   repository discovery and repository state checks
    /// - [`Self::allow_dirty`] still requires repository discovery, but returns
    ///   [`ModificationSafety::Safe`] and skips rejecting dirty or staged
    ///   changes
    /// - [`Self::allow_staged`] still requires repository discovery and change
    ///   queries, but dirty files remain unsafe
    ///
    /// When [`Self::check_entire_repository`] is disabled, the safety check is
    /// scoped to `path` after resolving it within the containing repository
    /// worktree. When enabled, the entire containing repository is checked.
    ///
    /// # Errors
    ///
    /// Returns an error if repository discovery fails, if `path` cannot be
    /// resolved for change queries, or if the backend fails to query the
    /// relevant changes.
    #[inline]
    pub fn check_safe_to_modify<P>(&self, path: P) -> Result<ModificationSafety, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        self.check_safe_to_modify_with_backend(path, &RealBackend)
    }

    fn check_safe_to_modify_with_backend<P, B>(
        &self,
        path: P,
        backend: &B,
    ) -> Result<ModificationSafety, ModifyGuardError>
    where
        P: AsRef<Path>,
        B: AllowOptionsBackend,
    {
        // Match `cargo fix` exactly:
        // - `--allow-no-vcs` skips repository discovery and repository state
        //   checks.
        // - `--allow-dirty` still requires repository discovery, but skips
        //   dirty and staged change checks.
        // - `--allow-staged` still requires repository discovery and change
        //   queries, but dirty files remain unsafe.

        let path = path.as_ref();

        if self.allow_no_vcs {
            return Ok(ModificationSafety::Safe);
        }

        let Some(repo) = backend.discover(path)? else {
            return Ok(UnsafeModificationReason::NoVcs.into());
        };

        if self.allow_dirty {
            return Ok(ModificationSafety::Safe);
        }

        let Some(changes) = self.find_changes(&repo, path)? else {
            return Ok(ModificationSafety::Safe);
        };

        let dirty_files = changes
            .files()
            .filter(|f| f.is_dirty())
            .map(|f| f.wt_path().to_owned())
            .collect::<Vec<_>>();

        if self.allow_staged {
            if !dirty_files.is_empty() {
                return Ok(UnsafeModificationReason::Dirty {
                    worktree: repo.worktree().to_owned(),
                    dirty_files,
                    staged_files: vec![],
                }
                .into());
            }
            return Ok(ModificationSafety::Safe);
        }

        let staged_files = changes
            .files()
            .filter(|f| f.is_staged())
            .map(|f| f.wt_path().to_owned())
            .collect::<Vec<_>>();

        if dirty_files.is_empty() {
            return Ok(UnsafeModificationReason::Staged {
                worktree: repo.worktree().to_owned(),
                staged_files,
            }
            .into());
        }

        Ok(UnsafeModificationReason::Dirty {
            worktree: repo.worktree().to_owned(),
            dirty_files,
            staged_files,
        }
        .into())
    }
}

trait AllowOptionsBackend {
    type Repo: AllowOptionsRepository;

    fn discover(&self, path: &Path) -> Result<Option<Self::Repo>, ModifyGuardError>;
}

trait AllowOptionsRepository {
    fn worktree(&self) -> &Path;
    fn resolve_path(&self, path: &Path) -> Result<PathBuf, ModifyGuardError>;
    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError>;
    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError>;
}

struct RealBackend;

impl AllowOptionsBackend for RealBackend {
    type Repo = Repository;

    fn discover(&self, path: &Path) -> Result<Option<Self::Repo>, ModifyGuardError> {
        Repository::discover(path)
    }
}

impl AllowOptionsRepository for Repository {
    fn worktree(&self) -> &Path {
        Repository::worktree(self)
    }
    fn resolve_path(&self, path: &Path) -> Result<PathBuf, ModifyGuardError> {
        Repository::resolve_path(self, path)
    }
    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        Repository::path_changes(self, wt_path)
    }
    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        Repository::repository_changes(self)
    }
}

/// Whether modification of the queried target is considered safe under the
/// current `--allow-*` policy.
///
/// This type describes the safety of modifying the queried target after
/// applying the configured policy.
#[expect(
    clippy::exhaustive_enums,
    reason = "Callers should exhaustively match the current outcomes; adding a new variant is an intentional breaking API change"
)]
#[derive(Debug)]
pub enum ModificationSafety {
    /// Modification of the queried target is considered safe.
    Safe,
    /// Modification of the queried target is considered unsafe.
    ///
    /// Contains the reason the modification is considered unsafe.
    Unsafe(UnsafeModificationReason),
}

/// The reason modification of the queried target is considered unsafe under
/// the current `--allow-*` policy.
///
/// This type explains why [`ModificationSafety::Unsafe`] was returned.
#[derive(Debug)]
#[expect(
    clippy::exhaustive_enums,
    reason = "Callers should exhaustively match the current outcomes; adding a new variant is an intentional breaking API change"
)]
pub enum UnsafeModificationReason {
    /// Modification is considered unsafe because no supported VCS repository
    /// was found for the queried target.
    NoVcs,
    /// Modification is considered unsafe because dirty files were found.
    ///
    /// `staged_files` is non-empty only when staged changes also make the
    /// modification unsafe.
    Dirty {
        /// The root directory of the containing repository worktree.
        worktree: PathBuf,
        /// Worktree-relative paths of dirty files that make the modification
        /// unsafe.
        ///
        /// This includes modified and untracked files.
        dirty_files: Vec<PathBuf>,
        /// Worktree-relative paths of staged files that also make the
        /// modification unsafe.
        staged_files: Vec<PathBuf>,
    },
    /// Modification is considered unsafe because staged changes were found.
    Staged {
        /// The root directory of the containing repository worktree.
        worktree: PathBuf,
        /// Worktree-relative paths of staged files that make the modification
        /// unsafe.
        staged_files: Vec<PathBuf>,
    },
}

impl From<UnsafeModificationReason> for ModificationSafety {
    #[inline]
    fn from(reason: UnsafeModificationReason) -> Self {
        Self::Unsafe(reason)
    }
}
