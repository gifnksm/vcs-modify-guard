use std::{fmt::Debug, path::Path};

use crate::{FileStatus, RepositoryStatus, VcsStatusError, error};

#[expect(
    clippy::unnecessary_wraps,
    reason = "When no VCS backend feature is enabled, this function intentionally returns Ok(None); the Result is kept to preserve a uniform interface with feature-enabled builds."
)]
pub(crate) fn discover(_path: &Path) -> Result<Option<Box<dyn VcsRepository>>, VcsStatusError> {
    Ok(None)
}

pub(crate) fn open(path: &Path) -> Result<Box<dyn VcsRepository>, VcsStatusError> {
    Err(error::NotARepositorySnafu { path }.build())
}

pub(crate) trait VcsRepository: Debug {
    fn workdir(&self) -> &Path;
    fn status(&self) -> Result<RepositoryStatus, VcsStatusError>;
    fn file_status(&self, path: &Path) -> Result<FileStatus, VcsStatusError>;
}

// assert that VcsRepository is dyn safe
const _: Option<&dyn VcsRepository> = None;
