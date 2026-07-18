use std::{
    fs::Metadata,
    io,
    path::{Path, PathBuf},
};

use snafu::{IntoError as _, ensure};

use crate::{VcsStatusError, error};

#[expect(
    clippy::allow_attributes,
    reason = "`allow` is necessary here because `unused` is only emitted when the feature is disabled"
)]
#[allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]
pub(crate) fn read_path_metadata(path: &Path) -> Result<Metadata, VcsStatusError> {
    path.metadata().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            error::PathNotFoundSnafu { path }.build()
        } else {
            error::InaccessiblePathSnafu { path }.into_error(source)
        }
    })
}

#[expect(
    clippy::allow_attributes,
    reason = "`allow` is necessary here because `unused` is only emitted when the feature is disabled"
)]
#[allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]
pub(crate) fn ensure_path_exists(path: &Path) -> Result<(), VcsStatusError> {
    let _metadata = read_path_metadata(path)?;
    Ok(())
}

#[expect(
    clippy::allow_attributes,
    reason = "`allow` is necessary here because `unused` is only emitted when the feature is disabled"
)]
#[allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]
pub(crate) fn ensure_path_is_directory(path: &Path) -> Result<(), VcsStatusError> {
    let metadata = read_path_metadata(path)?;
    ensure!(metadata.is_dir(), error::PathNotADirectorySnafu { path });
    Ok(())
}

#[expect(
    clippy::allow_attributes,
    reason = "`allow` is necessary here because `unused` is only emitted when the feature is disabled"
)]
#[allow(
    unused,
    reason = "avoids feature-dependent `unused` warnings without introducing more complex `cfg` conditions"
)]
pub(crate) fn normalize_worktree_relative_path(path: &Path) -> Result<PathBuf, VcsStatusError> {
    // `Path::components()` normalizes redundant separators and interior `.`
    // components, but preserves `..`. `CurDir` here therefore only comes
    // from a leading `.` path such as `./foo`, which we reject.
    let normalized = path
        .components()
        .map(|c| match c {
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::CurDir
            | std::path::Component::ParentDir => {
                Err(error::InvalidWorktreeRelativePathSnafu { path }.build())
            }
            std::path::Component::Normal(c) => Ok(c),
        })
        .collect::<Result<PathBuf, _>>()?;
    ensure!(
        !normalized.as_os_str().is_empty(),
        error::InvalidWorktreeRelativePathSnafu { path }
    );
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_worktree_relative_path_accepts_valid_paths() {
        let valid_paths = [
            ("valid/path", "valid/path"),
            ("another/valid/path", "another/valid/path"),
            ("file.txt", "file.txt"),
            ("path//with///slashes", "path/with/slashes"),
            ("path/./with/./current_dir", "path/with/current_dir"),
            #[cfg(windows)]
            (r"path\with\backslashes", "path/with/backslashes"),
        ];
        for (input, expected) in valid_paths {
            let actual = normalize_worktree_relative_path(Path::new(input)).unwrap();
            assert_eq!(&actual, expected);
        }
    }

    #[test]
    fn normalize_worktree_relative_path_rejects_invalid_paths() {
        let invalid_paths = [
            "",
            "/absolute/path",
            "./path/starts/with/current/directory",
            "./file.txt",
            "../path/starts/with/parent/directory",
            "path/with/../parent/directory",
            #[cfg(windows)]
            r"C:\absolute\path",
            #[cfg(windows)]
            r"\\network\share\path",
        ];
        for path in invalid_paths {
            normalize_worktree_relative_path(Path::new(path)).unwrap_err();
        }
    }
}
