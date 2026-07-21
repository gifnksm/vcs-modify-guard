#![allow(
    dead_code,
    reason = "shared utility helpers may be unused in some backend feature sets, and handling that with finer-grained cfgs would add unnecessary complexity"
)]

use std::{
    collections::VecDeque,
    fs::Metadata,
    io,
    path::{Component, Path, PathBuf, StripPrefixError},
};

use snafu::{IntoError as _, OptionExt as _, ensure};

use crate::{ModifyGuardError, error};

pub(crate) fn read_path_metadata(path: &Path) -> Result<Metadata, ModifyGuardError> {
    path.metadata().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            error::PathNotFoundSnafu { path }.build()
        } else {
            error::InaccessiblePathSnafu { path }.into_error(source)
        }
    })
}

pub(crate) fn ensure_path_exists(path: &Path) -> Result<(), ModifyGuardError> {
    let _metadata = read_path_metadata(path)?;
    Ok(())
}

pub(crate) fn ensure_path_is_directory(path: &Path) -> Result<(), ModifyGuardError> {
    let metadata = read_path_metadata(path)?;
    ensure!(metadata.is_dir(), error::PathNotADirectorySnafu { path });
    Ok(())
}

pub(crate) fn ensure_path_is_file(path: &Path) -> Result<(), ModifyGuardError> {
    let metadata = read_path_metadata(path)?;
    ensure!(metadata.is_file(), error::PathNotAFileSnafu { path });
    Ok(())
}

fn canonicalize_path<P>(path: P) -> Result<PathBuf, ModifyGuardError>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    dunce::canonicalize(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            error::PathNotFoundSnafu { path }.build()
        } else {
            error::CanonicalizePathSnafu { path }.into_error(source)
        }
    })
}

#[derive(Debug)]
pub(crate) enum NormalizedWorktreePath {
    Existing(PathBuf),
    Missing(PathBuf),
}

impl AsRef<Path> for NormalizedWorktreePath {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<NormalizedWorktreePath> for PathBuf {
    #[inline]
    fn from(value: NormalizedWorktreePath) -> Self {
        match value {
            NormalizedWorktreePath::Existing(path) | NormalizedWorktreePath::Missing(path) => path,
        }
    }
}

impl NormalizedWorktreePath {
    fn new<P>(path: P) -> Result<Self, ModifyGuardError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let mut components = path.components();
        let mut trimmed_components = VecDeque::new();
        loop {
            match dunce::canonicalize(components.as_path()) {
                Ok(mut canonicalized) => {
                    if trimmed_components.is_empty() {
                        return Ok(Self::Existing(canonicalized));
                    }
                    canonicalized.extend(trimmed_components);
                    return Ok(Self::Missing(canonicalized));
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(error::CanonicalizePathSnafu {
                        path: components.as_path(),
                    }
                    .into_error(source));
                }
            }
            let Some(comp) = components.next_back() else {
                return Err(error::InvalidWorktreeRelativePathSnafu { path }.build());
            };
            ensure!(
                matches!(comp, Component::Normal(_)),
                error::InvalidWorktreeRelativePathSnafu { path }
            );
            trimmed_components.push_front(comp);
        }
    }

    pub(crate) fn as_path(&self) -> &Path {
        match self {
            NormalizedWorktreePath::Existing(path) | NormalizedWorktreePath::Missing(path) => path,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.as_path().as_os_str().is_empty()
    }

    pub(crate) fn strip_prefix<P>(&self, base: P) -> Result<Self, StripPrefixError>
    where
        P: AsRef<Path>,
    {
        let stripped = self.as_path().strip_prefix(base)?.to_owned();
        let stripped = match self {
            Self::Existing(_) => Self::Existing(stripped),
            Self::Missing(_) => Self::Missing(stripped),
        };
        Ok(stripped)
    }
}

pub(crate) fn normalize_to_worktree_path<P, Q>(
    worktree_path: P,
    path: Q,
) -> Result<NormalizedWorktreePath, ModifyGuardError>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let worktree_path = worktree_path.as_ref();
    let path = path.as_ref();
    let worktree_path = canonicalize_path(worktree_path)?;
    let entry_path = NormalizedWorktreePath::new(worktree_path.join(path))?;
    let normalized = entry_path
        .strip_prefix(&worktree_path)
        .ok()
        .context(error::InvalidWorktreeRelativePathSnafu { path })?;
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use assert_fs::prelude::*;
    use rstest::*;

    use super::*;
    use crate::testing::PathInTempDir;

    #[track_caller]
    fn assert_existing<P>(actual: NormalizedWorktreePath, expected: P)
    where
        P: AsRef<Path>,
    {
        assert_matches!(actual, NormalizedWorktreePath::Existing(p) if p == expected.as_ref());
    }

    #[track_caller]
    fn assert_missing<P>(actual: NormalizedWorktreePath, expected: P)
    where
        P: AsRef<Path>,
    {
        assert_matches!(actual, NormalizedWorktreePath::Missing(p) if p == expected.as_ref());
    }

    #[fixture]
    fn file_tree() -> PathInTempDir {
        let path = PathInTempDir::new();
        path.child("a/b/c/d.txt").touch().unwrap();
        path
    }

    #[cfg(unix)]
    #[fixture]
    fn file_tree_with_symlink() -> PathInTempDir {
        use std::fs;

        let path = PathInTempDir::new();
        fs::create_dir_all(path.child("a/b/c")).unwrap();
        std::os::unix::fs::symlink("c", path.path().join("a/b/L")).unwrap();
        path
    }

    #[rstest]
    fn normalize_to_worktree_path_canonicalizes_existing_path(file_tree: PathInTempDir) {
        let path = file_tree;
        let subpaths = [
            "a/b/c/d.txt",
            "a/../a/b/../b/c/d.txt",
            "a//b//c//d.txt",
            "a/./b//c/d.txt",
        ];
        for subpath in subpaths {
            let normalized = normalize_to_worktree_path(&path, path.child(subpath)).unwrap();
            assert_existing(normalized, "a/b/c/d.txt");
        }
    }

    #[rstest]
    fn normalize_to_worktree_path_partially_canonicalizes_path_with_missing_leaf(
        file_tree: PathInTempDir,
    ) {
        let path = file_tree;
        let subpaths = [
            "a/b/c/X.txt",
            "a/../a/b/../b/c/X.txt",
            "a//b//c//X.txt",
            "a/./b//c/X.txt",
        ];
        for subpath in subpaths {
            let normalized = normalize_to_worktree_path(&path, path.child(subpath)).unwrap();
            assert_missing(normalized, "a/b/c/X.txt");
        }
    }

    #[rstest]
    fn normalize_to_worktree_path_partially_canonicalizes_path_with_missing_leaves(
        file_tree: PathInTempDir,
    ) {
        let path = file_tree;
        let normalized = normalize_to_worktree_path(&path, path.child("a/b/c/X/Y/Z.txt")).unwrap();
        assert_missing(normalized, "a/b/c/X/Y/Z.txt");
    }

    #[cfg(unix)]
    #[rstest]
    fn normalize_to_worktree_path_resolves_symlinks_in_existing_prefix(
        file_tree_with_symlink: PathInTempDir,
    ) {
        let path = file_tree_with_symlink;
        let normalized = normalize_to_worktree_path(&path, path.child("a/b/L/X.txt")).unwrap();
        assert_missing(normalized, "a/b/c/X.txt");
    }

    #[rstest]
    fn normalize_to_worktree_path_rejects_path_outside_worktree(file_tree: PathInTempDir) {
        let path = file_tree;
        let err = normalize_to_worktree_path(&path, path.child("a/../../X.txt")).unwrap_err();
        assert_matches!(err, ModifyGuardError::InvalidWorktreeRelativePath { .. });
    }

    // `normalize_to_worktree_path` trims missing trailing components only after
    // `dunce::canonicalize` fails. On Unix-like platforms, canonicalization of
    // `a/X/../../X.txt` still fails while trying to traverse the missing `X`
    // component, so trimming eventually reaches `..` and rejects the path.
    #[cfg(not(windows))]
    #[rstest]
    fn normalize_to_worktree_path_rejects_dotdot_left_in_missing_suffix(file_tree: PathInTempDir) {
        let path = file_tree;
        let err = normalize_to_worktree_path(&path, path.child("a/X/../../X.txt")).unwrap_err();
        assert_matches!(err, ModifyGuardError::InvalidWorktreeRelativePath { .. });
    }

    // `normalize_to_worktree_path` trims missing trailing components only after
    // `dunce::canonicalize` fails. On Windows, canonicalization of
    // `a/X/../../X.txt` succeeds earlier because the Windows path machinery
    // lexically resolves the `..` components before existence checks, so
    // trimming never reaches them and the remaining path is accepted as
    // `X.txt`.
    #[cfg(windows)]
    #[rstest]
    fn normalize_to_worktree_path_resolves_dotdot_before_missing_suffix(file_tree: PathInTempDir) {
        let path = file_tree;
        let normalized = normalize_to_worktree_path(&path, path.child("a/X/../../X.txt")).unwrap();
        assert_missing(normalized, "X.txt");
    }

    // Even on Windows, canonicalization of `a/X/X/X/X/../../` still fails
    // before the trailing `..` components are eliminated, so trimming reaches
    // `..` and the path is rejected on all platforms.
    #[rstest]
    fn normalize_to_worktree_path_rejects_unresolved_dotdot_in_missing_suffix(
        file_tree: PathInTempDir,
    ) {
        let path = file_tree;
        let err = normalize_to_worktree_path(&path, path.child("a/X/X/X/X/../../")).unwrap_err();
        assert_matches!(err, ModifyGuardError::InvalidWorktreeRelativePath { .. });
    }

    #[rstest]
    fn normalize_to_worktree_path_resolves_empty_path(file_tree: PathInTempDir) {
        let path = file_tree;
        let normalized = normalize_to_worktree_path(&path, path.child("")).unwrap();
        assert_existing(normalized, "");
    }

    #[rstest]
    fn normalize_to_worktree_path_resolves_current_dir_as_empty_path(file_tree: PathInTempDir) {
        let path = file_tree;
        let normalized = normalize_to_worktree_path(&path, path.child(".")).unwrap();
        assert_existing(normalized, "");
        let normalized = normalize_to_worktree_path(&path, path.child("./")).unwrap();
        assert_existing(normalized, "");
    }
}
