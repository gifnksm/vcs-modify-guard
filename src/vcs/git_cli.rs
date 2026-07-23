use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output},
    str::Utf8Error,
};

use snafu::{OptionExt as _, ResultExt as _, Snafu, ensure};

use crate::{
    ModifyGuardError, error,
    repository::{FileChange, RepositoryChanges},
    util::{self, NormalizedPath},
    vcs::VcsBackend,
};

use super::VcsRepository;

pub(super) const BACKEND: GitCliBackend = GitCliBackend;

#[derive(Debug)]
pub(super) struct GitCliBackend;

/// Errors returned by `git-cli` backend operations.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum GitCliBackendError {
    /// Executing a `git` command failed.
    #[snafu(display(
        "failed to execute git command: {}{}",
        program.display(),
        args.iter().fold(String::new(), |mut output, arg| {
            let _ = write!(&mut output, " {}", arg.display());
            output
        })
    ))]
    GitCommand {
        /// The underlying error from executing the `git` command.
        source: io::Error,
        /// The `git` program that was executed.
        program: OsString,
        /// The arguments passed to the `git` command.
        args: Vec<OsString>,
    },
    /// A `git` command returned a non-zero exit status.
    #[snafu(display("git command returned non-zero exit status: {status}"))]
    GitExitStatus {
        /// The exit status returned by the `git` command.
        status: ExitStatus,
    },
    /// Converting the output of a `git` command to UTF-8 failed.
    #[snafu(display("failed to convert git command output to UTF-8"))]
    InvalidUtf8 {
        /// The underlying error from converting the `git` command output to UTF-8.
        source: Utf8Error,
    },
    /// An invalid status entry was encountered in the output of `git status`.
    #[snafu(display("invalid status entry in git status output: {entry:?}"))]
    InvalidGitStatus {
        /// The invalid status entry from the `git status` output.
        entry: Vec<u8>,
    },
    /// An invalid rev-parse output was encountered.
    #[snafu(display("invalid rev-parse output: {output:?}"))]
    InvalidRevParse {
        /// The invalid output from the `git rev-parse` command.
        output: Vec<u8>,
    },
    /// A file query matched a different path than requested.
    #[snafu(display(
        "git file query for {} matched a different path: {}",
        requested.display(),
        actual.display()
    ))]
    AmbiguousFilePath {
        /// The worktree-relative path requested by the file query.
        requested: PathBuf,
        /// The worktree-relative path returned by Git instead.
        actual: PathBuf,
    },
    /// A path was expected to have a parent directory, but it did not.
    #[snafu(display("path has no parent directory: {}", git_dir.display()))]
    NoGitDirParent {
        /// The path that was expected to have a parent directory.
        git_dir: PathBuf,
    },
    /// No worktree listed by Git matched the repository administrative directory.
    #[snafu(display("no listed worktree matched git dir: {}", git_dir.display()))]
    NoWorktreeForGitDir {
        /// The repository administrative directory that could not be mapped back to a worktree.
        git_dir: PathBuf,
    },
}

impl From<GitCliBackendError> for ModifyGuardError {
    #[inline]
    fn from(source: GitCliBackendError) -> Self {
        Self::Backend {
            source: source.into(),
        }
    }
}

impl VcsBackend for GitCliBackend {
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

        let Some(is_bare) = repo_is_bare(path)? else {
            return Ok(None);
        };
        ensure!(!is_bare, error::RepositoryWithoutWorktreeSnafu { path });

        let worktree = if repo_is_inside_git_dir(path)? {
            let git_dir = repo_absolute_git_dir(path)?;
            repo_worktree_from_git_dir(&git_dir)?
        } else {
            repo_toplevel(path)?
        };

        Ok(Some(Box::new(GitCliRepository { worktree })))
    }

    fn open(&self, path: &Path) -> Result<Option<Box<dyn VcsRepository>>, ModifyGuardError> {
        util::ensure_path_is_directory(path)?;

        let Some(is_bare) = repo_is_bare(path)? else {
            return Ok(None);
        };
        if is_bare {
            let git_dir = repo_absolute_git_dir(path)?;
            if is_same_path(&git_dir, path) {
                return Err(error::RepositoryWithoutWorktreeSnafu { path }.build());
            }
            return Ok(None);
        }

        if repo_is_inside_git_dir(path)? {
            let git_dir = repo_absolute_git_dir(path)?;
            if is_same_path(&git_dir, path) {
                let worktree = repo_worktree_from_git_dir(&git_dir)?;
                return Ok(Some(Box::new(GitCliRepository { worktree })));
            }
            return Ok(None);
        }

        let prefix = run_git(["rev-parse", "--show-prefix"], path)?;
        let prefix = parse_stdout_as_path(&prefix)?;
        if !prefix.as_os_str().is_empty() {
            return Ok(None);
        }

        let worktree = repo_toplevel(path)?;
        Ok(Some(Box::new(GitCliRepository { worktree })))
    }
}

#[derive(Debug)]
struct GitCliRepository {
    worktree: PathBuf,
}

impl VcsRepository for GitCliRepository {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn repository_changes(&self) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        let file_changes = self.path_files_changes(None)?;
        Ok(RepositoryChanges::new(file_changes))
    }

    fn path_changes(&self, wt_path: &Path) -> Result<Option<RepositoryChanges>, ModifyGuardError> {
        let wt_path = util::normalize_worktree_path(&self.worktree, wt_path)?;
        let file_changes = self.path_files_changes(Some(&wt_path))?;
        Ok(RepositoryChanges::new(file_changes))
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
        let mut file_changes = self.path_files_changes(Some(&wt_path))?;
        let Some(change) = file_changes.pop() else {
            return Ok(None);
        };
        // `git status <pathspec>` treats a missing directory path as a prefix match
        // and may return changes below it. `file_change()` needs file semantics like
        // `git2::Repository::status_file`, so accept only an exact path match.
        if change.wt_path() == wt_path.as_path() {
            return Ok(Some(change));
        }
        Err(AmbiguousFilePathSnafu {
            requested: wt_path.as_path().to_path_buf(),
            actual: change.wt_path().to_path_buf(),
        }
        .build()
        .into())
    }
}

impl GitCliRepository {
    fn path_files_changes(
        &self,
        wt_path: Option<&NormalizedPath>,
    ) -> Result<Vec<FileChange>, ModifyGuardError> {
        let pathspec = wt_path
            .as_ref()
            .filter(|wt_path| !wt_path.is_empty())
            .map(|wt_path| {
                let mut p = OsString::from(":(literal)");
                p.push(wt_path.as_path().as_os_str());
                p
            });
        let args = [
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--no-ignored",
            "--untracked-files=all",
        ]
        .into_iter()
        .map(|s| Cow::Borrowed(OsStr::new(s)))
        .chain(pathspec.map(Cow::Owned));
        let statuses = run_git(args, &self.worktree)?;
        let statuses = parse_stdout_as_bytes(&statuses);
        let statuses = parse_git_status(statuses)?;
        let statuses = statuses
            .into_iter()
            .filter_map(StatusEntry::build)
            .peekable()
            .collect::<Vec<_>>();
        if statuses.is_empty()
            && let Some(NormalizedPath::Missing(wt_path)) = &wt_path
        {
            return Err(error::PathNotFoundSnafu { path: wt_path }.build());
        }
        Ok(statuses)
    }
}

const REPO_CONTEXT_ENV_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
];

fn git_command(current_dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    // Make this backend interpret the caller's path the same way as the
    // libgit2 backend. These vars can redirect Git to a different repository,
    // worktree, common dir, or index than the path being queried.
    for env_var in REPO_CONTEXT_ENV_VARS {
        cmd.env_remove(env_var);
    }
    cmd.current_dir(current_dir);
    cmd
}

fn run_git_without_status_check<I, S>(
    args: I,
    current_dir: &Path,
) -> Result<Output, GitCliBackendError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = git_command(current_dir);
    cmd.args(args);
    let output = cmd.output().with_context(|_| GitCommandSnafu {
        program: cmd.get_program(),
        args: cmd.get_args().map(ToOwned::to_owned).collect::<Vec<_>>(),
    })?;
    Ok(output)
}

fn run_git<I, S>(args: I, current_dir: &Path) -> Result<Output, GitCliBackendError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_without_status_check(args, current_dir)?;
    ensure!(
        output.status.success(),
        GitExitStatusSnafu {
            status: output.status
        }
    );
    Ok(output)
}

fn trim_trailing_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn parse_stdout_as_bytes(output: &Output) -> &[u8] {
    trim_trailing_newline(&output.stdout)
}

fn parse_stdout_as_os_str(output: &Output) -> Result<&OsStr, GitCliBackendError> {
    let s = parse_stdout_as_bytes(output);
    util::bytes_to_os_str(s).context(InvalidUtf8Snafu)
}

fn parse_stdout_as_path(output: &Output) -> Result<&Path, GitCliBackendError> {
    let path = parse_stdout_as_os_str(output)?;
    Ok(Path::new(path))
}

fn parse_stdout_as_bool(output: &Output) -> Result<bool, GitCliBackendError> {
    let s = parse_stdout_as_bytes(output);
    match s {
        b"true" => Ok(true),
        b"false" => Ok(false),
        bytes => Err(InvalidRevParseSnafu { output: bytes }.build()),
    }
}

fn repo_is_bare(path: &Path) -> Result<Option<bool>, GitCliBackendError> {
    let output = run_git_without_status_check(["rev-parse", "--is-bare-repository"], path)?;
    if !output.status.success() {
        return Ok(None);
    }
    let is_bare = parse_stdout_as_bool(&output)?;
    Ok(Some(is_bare))
}

fn repo_is_inside_git_dir(path: &Path) -> Result<bool, GitCliBackendError> {
    let output = run_git(["rev-parse", "--is-inside-git-dir"], path)?;
    parse_stdout_as_bool(&output)
}

fn repo_absolute_git_dir(path: &Path) -> Result<PathBuf, GitCliBackendError> {
    let output = run_git(["rev-parse", "--absolute-git-dir"], path)?;
    Ok(parse_stdout_as_path(&output)?.to_path_buf())
}

fn repo_toplevel(path: &Path) -> Result<PathBuf, GitCliBackendError> {
    let output = run_git(["rev-parse", "--show-toplevel"], path)?;
    Ok(parse_stdout_as_path(&output)?.to_path_buf())
}

fn run_git_for_git_dir<I, S>(git_dir: &Path, args: I) -> Result<Output, GitCliBackendError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let query_dir = git_dir.parent().context(NoGitDirParentSnafu {
        git_dir: git_dir.to_path_buf(),
    })?;
    let mut git_dir_arg = OsString::from("--git-dir=");
    git_dir_arg.push(git_dir.as_os_str());
    let args = std::iter::once(Cow::Owned(git_dir_arg)).chain(
        args.into_iter()
            .map(|arg| Cow::Owned(arg.as_ref().to_os_string())),
    );
    run_git(args, query_dir)
}

fn repo_common_git_dir(git_dir: &Path) -> Result<PathBuf, GitCliBackendError> {
    let output = run_git_for_git_dir(
        git_dir,
        [OsStr::new("rev-parse"), OsStr::new("--git-common-dir")],
    )?;
    let common_dir = parse_stdout_as_path(&output)?;
    if common_dir.is_absolute() {
        return Ok(common_dir.to_path_buf());
    }
    let query_dir = git_dir.parent().context(NoGitDirParentSnafu {
        git_dir: git_dir.to_path_buf(),
    })?;
    Ok(query_dir.join(common_dir))
}

// Parse the documented stable porcelain format rather than reading
// `$GIT_DIR/worktrees/*/gitdir` directly. `git-worktree(1)` says `--porcelain`
// output "will remain stable across Git versions and regardless of user
// configuration", and "The first attribute of a worktree is always `worktree'".
// <https://git-scm.com/docs/git-worktree>
fn parse_worktree_list_porcelain(output: &[u8]) -> Result<Vec<PathBuf>, GitCliBackendError> {
    let mut worktrees = vec![];
    for field in output.split(|&byte| byte == b'\0') {
        let Some(path) = field.strip_prefix(b"worktree ") else {
            continue;
        };
        let path = util::bytes_to_os_str(path).context(InvalidUtf8Snafu)?;
        worktrees.push(PathBuf::from(path));
    }
    Ok(worktrees)
}

fn repo_worktree_from_git_dir(git_dir: &Path) -> Result<PathBuf, GitCliBackendError> {
    let common_git_dir = repo_common_git_dir(git_dir)?;
    let worktrees = run_git_for_git_dir(
        &common_git_dir,
        [
            OsStr::new("worktree"),
            OsStr::new("list"),
            OsStr::new("--porcelain"),
            OsStr::new("-z"),
        ],
    )?;
    let worktrees = parse_worktree_list_porcelain(parse_stdout_as_bytes(&worktrees))?;
    // Reverse-map the administrative git dir back to the owning worktree by
    // asking Git for every listed worktree's absolute git dir and comparing the
    // resolved paths. This keeps the implementation CLI-based for linked
    // worktrees instead of depending on `.git` file layout details.
    for worktree in worktrees {
        let candidate_git_dir = repo_absolute_git_dir(&worktree)?;
        if is_same_path(&candidate_git_dir, git_dir) {
            return Ok(worktree);
        }
    }
    NoWorktreeForGitDirSnafu {
        git_dir: git_dir.to_path_buf(),
    }
    .fail()
}

fn is_same_path(path1: &Path, path2: &Path) -> bool {
    if path1.components() == path2.components() {
        return true;
    }
    match (path1.canonicalize(), path2.canonicalize()) {
        (Ok(canon1), Ok(canon2)) => canon1 == canon2,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeKind {
    Unmodified,
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    UpdatedButUnmerged,
    Untracked,
}

impl ChangeKind {
    fn from_byte(c: u8) -> Option<Self> {
        match c {
            b' ' => Some(Self::Unmodified),
            b'M' => Some(Self::Modified),
            b'T' => Some(Self::TypeChanged),
            b'A' => Some(Self::Added),
            b'D' => Some(Self::Deleted),
            b'R' => Some(Self::Renamed),
            b'C' => Some(Self::Copied),
            b'U' => Some(Self::UpdatedButUnmerged),
            b'?' => Some(Self::Untracked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusEntry<'a> {
    index: ChangeKind,
    worktree: ChangeKind,
    wt_path: &'a OsStr,
}

fn parse_git_status(status: &[u8]) -> Result<Vec<StatusEntry<'_>>, GitCliBackendError> {
    let mut changes = vec![];
    for entry in status.split(|c| *c == b'\0') {
        if entry.is_empty() {
            continue;
        }
        let mut cs = entry.iter();
        let index = cs
            .next()
            .copied()
            .and_then(ChangeKind::from_byte)
            .context(InvalidGitStatusSnafu { entry })?;
        let worktree = cs
            .next()
            .copied()
            .and_then(ChangeKind::from_byte)
            .context(InvalidGitStatusSnafu { entry })?;
        let space = cs
            .next()
            .copied()
            .context(InvalidGitStatusSnafu { entry })?;
        ensure!(space == b' ', InvalidGitStatusSnafu { entry });
        let Some(wt_path) = util::bytes_to_os_str(cs.as_slice()).ok() else {
            // Match the libgit2 backend's aggregate queries by skipping status
            // entries whose paths cannot be represented on this platform.
            continue;
        };
        let entry = StatusEntry {
            index,
            worktree,
            wt_path,
        };
        changes.push(entry);
    }
    Ok(changes)
}

impl StatusEntry<'_> {
    fn build(self) -> Option<FileChange> {
        let StatusEntry {
            index,
            worktree,
            wt_path,
        } = self;
        let (dirty, staged) = if index == ChangeKind::Untracked || worktree == ChangeKind::Untracked
        {
            (true, false)
        } else {
            (
                worktree != ChangeKind::Unmodified,
                index != ChangeKind::Unmodified,
            )
        };
        (dirty || staged).then(|| FileChange {
            wt_path: PathBuf::from(wt_path),
            dirty,
            staged,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_status_returns_file_changes() {
        use ChangeKind::*;

        let status = b"   clean.txt\0M  staged.txt\0";
        let changes = parse_git_status(status).unwrap();
        assert_eq!(
            changes,
            [
                StatusEntry {
                    index: Unmodified,
                    worktree: Unmodified,
                    wt_path: OsStr::new("clean.txt")
                },
                StatusEntry {
                    index: Modified,
                    worktree: Unmodified,
                    wt_path: OsStr::new("staged.txt")
                },
            ]
        );
    }
}
