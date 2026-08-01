#![allow(
    dead_code,
    reason = "shared utility helpers may be unused in some backend feature sets, and handling that with finer-grained cfgs would add unnecessary complexity"
)]

use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs::Metadata,
    io,
    path::{Component, Path, PathBuf},
    str::Utf8Error,
};

use snafu::{IntoError as _, OptionExt as _, ensure};

use crate::{ModifyGuardError, error};

#[cfg(unix)]
#[expect(clippy::unnecessary_wraps, reason = "Unix implementation cannot fail")]
pub(crate) fn bytes_to_os_str(bytes: &[u8]) -> Result<&OsStr, Utf8Error> {
    use std::os::unix::ffi::OsStrExt as _;
    Ok(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
pub(crate) fn bytes_to_os_str(bytes: &[u8]) -> Result<&OsStr, Utf8Error> {
    let s = str::from_utf8(bytes)?;
    Ok(OsStr::new(s))
}

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

pub(crate) fn canonicalize_path<P>(path: P) -> Result<PathBuf, ModifyGuardError>
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
pub(crate) enum WorktreeRelativePath {
    Existing(PathBuf),
    Missing(PathBuf),
}

impl AsRef<Path> for WorktreeRelativePath {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<WorktreeRelativePath> for PathBuf {
    #[inline]
    fn from(value: WorktreeRelativePath) -> Self {
        match value {
            WorktreeRelativePath::Existing(path) | WorktreeRelativePath::Missing(path) => path,
        }
    }
}

#[derive(Debug)]
struct NormalizedPath {
    existing: bool,
    path: PathBuf,
}

impl NormalizedPath {
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
                        return Ok(Self {
                            existing: true,
                            path: canonicalized,
                        });
                    }
                    canonicalized.extend(trimmed_components);
                    return Ok(Self {
                        existing: false,
                        path: canonicalized,
                    });
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
                return Err(error::ResolveAsWorktreeRelativePathSnafu { path }.build());
            };
            ensure!(
                matches!(comp, Component::Normal(_)),
                error::ResolveAsWorktreeRelativePathSnafu { path }
            );
            trimmed_components.push_front(comp);
        }
    }
}

#[cfg(not(windows))]
fn to_unix_separators(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn to_unix_separators(path: &Path) -> PathBuf {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt as _, OsStringExt as _},
    };

    let wide = path
        .as_os_str()
        .encode_wide()
        .map(|w| {
            if w == u16::from(b'\\') {
                u16::from(b'/')
            } else {
                w
            }
        })
        .collect::<Vec<_>>();
    PathBuf::from(OsString::from_wide(&wide))
}

impl WorktreeRelativePath {
    pub(crate) fn from_path<P, Q>(worktree_path: P, path: Q) -> Result<Self, ModifyGuardError>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let path = path.as_ref();
        let worktree_path = canonicalize_path(worktree_path)?;
        let NormalizedPath {
            existing,
            path: normalized_path,
        } = NormalizedPath::new(path)?;
        let relative_path = normalized_path
            .strip_prefix(&worktree_path)
            .ok()
            .context(error::ResolveAsWorktreeRelativePathSnafu { path })?;
        let relative_path = to_unix_separators(relative_path);
        if existing {
            Ok(Self::Existing(relative_path))
        } else {
            Ok(Self::Missing(relative_path))
        }
    }

    pub(crate) fn from_wt_path<P, Q>(worktree_path: P, wt_path: Q) -> Result<Self, ModifyGuardError>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let worktree_path = worktree_path.as_ref();
        let wt_path = wt_path.as_ref();
        ensure!(
            wt_path.is_relative(),
            error::ResolveAsWorktreeRelativePathSnafu { path: wt_path }
        );
        Self::from_path(worktree_path, worktree_path.join(wt_path))
    }

    pub(crate) fn as_path(&self) -> &Path {
        match self {
            WorktreeRelativePath::Existing(path) | WorktreeRelativePath::Missing(path) => path,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.as_path().as_os_str().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use assert_fs::prelude::*;
    use rstest::*;

    use super::*;
    use crate::testing::PathInTempDir;

    #[track_caller]
    fn assert_existing<P>(actual: WorktreeRelativePath, expected: P)
    where
        P: AsRef<Path>,
    {
        assert_matches!(actual, WorktreeRelativePath::Existing(p) if p == expected.as_ref());
    }

    #[track_caller]
    fn assert_missing<P>(actual: WorktreeRelativePath, expected: P)
    where
        P: AsRef<Path>,
    {
        assert_matches!(actual, WorktreeRelativePath::Missing(p) if p == expected.as_ref());
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
    fn worktree_relative_path_canonicalizes_existing_path(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let wt_paths = [
            "a/b/c/d.txt",
            "a/../a/b/../b/c/d.txt",
            "a//b//c//d.txt",
            "a/./b//c/d.txt",
        ];
        for wt_path in wt_paths {
            let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, wt_path).unwrap();
            assert_existing(wt_path, "a/b/c/d.txt");
        }
    }

    #[rstest]
    fn worktree_relative_path_partially_canonicalizes_path_with_missing_leaf(
        file_tree: PathInTempDir,
    ) {
        let worktree_path = file_tree;
        let wt_paths = [
            "a/b/c/X.txt",
            "a/../a/b/../b/c/X.txt",
            "a//b//c//X.txt",
            "a/./b//c/X.txt",
        ];
        for wt_path in wt_paths {
            let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, wt_path).unwrap();
            assert_missing(wt_path, "a/b/c/X.txt");
        }
    }

    #[rstest]
    fn worktree_relative_path_partially_canonicalizes_path_with_missing_leaves(
        file_tree: PathInTempDir,
    ) {
        let worktree_path = file_tree;
        let wt_path =
            WorktreeRelativePath::from_wt_path(&worktree_path, "a/b/c/X/Y/Z.txt").unwrap();
        assert_missing(wt_path, "a/b/c/X/Y/Z.txt");
    }

    #[cfg(unix)]
    #[rstest]
    fn worktree_relative_path_resolves_symlinks_in_existing_prefix(
        file_tree_with_symlink: PathInTempDir,
    ) {
        let worktree_path = file_tree_with_symlink;
        let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, "a/b/L/X.txt").unwrap();
        assert_missing(wt_path, "a/b/c/X.txt");
    }

    #[rstest]
    fn worktree_relative_path_rejects_path_outside_worktree(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let err = WorktreeRelativePath::from_wt_path(&worktree_path, "a/../../X.txt").unwrap_err();
        assert_matches!(err, ModifyGuardError::ResolveAsWorktreeRelativePath { .. });
    }

    // `WorktreeRelativePath::from_wt_path` trims missing trailing components only after
    // `dunce::canonicalize` fails. On Unix-like platforms, canonicalization of
    // `a/X/../../X.txt` still fails while trying to traverse the missing `X`
    // component, so trimming eventually reaches `..` and rejects the path.
    #[cfg(not(windows))]
    #[rstest]
    fn worktree_relative_path_rejects_dotdot_left_in_missing_suffix(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let err =
            WorktreeRelativePath::from_wt_path(&worktree_path, "a/X/../../X.txt").unwrap_err();
        assert_matches!(err, ModifyGuardError::ResolveAsWorktreeRelativePath { .. });
    }

    // `WorktreeRelativePath::from_wt_path` trims missing trailing components only after
    // `dunce::canonicalize` fails. On Windows, canonicalization of
    // `a/X/../../X.txt` succeeds earlier because the Windows path machinery
    // lexically resolves the `..` components before existence checks, so
    // trimming never reaches them and the remaining path is accepted as
    // `X.txt`.
    #[cfg(not(unix))]
    #[rstest]
    fn worktree_relative_path_resolves_dotdot_before_missing_suffix(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let wt_path =
            WorktreeRelativePath::from_wt_path(&worktree_path, "a/X/../../X.txt").unwrap();
        assert_missing(wt_path, "X.txt");
    }

    // Even on Windows, canonicalization of `a/X/X/X/X/../../` still fails
    // before the trailing `..` components are eliminated, so trimming reaches
    // `..` and the path is rejected on all platforms.
    #[rstest]
    fn worktree_relative_path_rejects_unresolved_dotdot_in_missing_suffix(
        file_tree: PathInTempDir,
    ) {
        let worktree_path = file_tree;
        let err =
            WorktreeRelativePath::from_wt_path(&worktree_path, "a/X/X/X/X/../../").unwrap_err();
        assert_matches!(err, ModifyGuardError::ResolveAsWorktreeRelativePath { .. });
    }

    #[rstest]
    fn worktree_relative_path_resolves_empty_path(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, "").unwrap();
        assert_existing(wt_path, "");
    }

    #[rstest]
    fn worktree_relative_path_resolves_current_dir_as_empty_path(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, ".").unwrap();
        assert_existing(wt_path, "");
        let wt_path = WorktreeRelativePath::from_wt_path(&worktree_path, "./").unwrap();
        assert_existing(wt_path, "");
    }

    #[rstest]
    fn worktree_relative_path_rejects_absolute_path(file_tree: PathInTempDir) {
        let worktree_path = file_tree;
        let err =
            WorktreeRelativePath::from_wt_path(&worktree_path, worktree_path.child("/a/b/c/d.txt"))
                .unwrap_err();
        assert_matches!(err, ModifyGuardError::ResolveAsWorktreeRelativePath { .. });
    }
}
