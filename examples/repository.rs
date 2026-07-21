//! Demonstrates how a CLI tool can implement `--allow-dirty`,
//! `--allow-staged`, and `--allow-no-vcs` options with behavior
//! equivalent to `cargo fix`.

use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Parser;
use vcs_modify_guard::repository::Repository;

#[derive(Debug, Parser)]
struct Args {
    /// Process code even if a VCS was not detected.
    #[arg(long)]
    allow_no_vcs: bool,

    /// Process code even if the containing repository has modified, staged,
    /// or untracked files.
    #[arg(long)]
    allow_dirty: bool,

    /// Process code even if the containing repository has staged changes.
    #[arg(long)]
    allow_staged: bool,

    /// Target directory to process. Defaults to the current working directory.
    /// The repository containing this directory will be checked.
    #[arg(long)]
    target_dir: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let target_dir = args.target_dir.as_deref().unwrap_or_else(|| Path::new("."));
    let options = Options {
        allow_no_vcs: args.allow_no_vcs,
        allow_staged: args.allow_staged,
        allow_dirty: args.allow_dirty,
    };

    ensure_safe_to_modify(target_dir, &options)?;

    eprintln!("Proceeding...");

    Ok(())
}

struct Options {
    allow_no_vcs: bool,
    allow_dirty: bool,
    allow_staged: bool,
}

fn ensure_safe_to_modify(target_dir: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    // Match `cargo fix` exactly:
    // - `--allow-no-vcs` allows running even when no repository is found.
    // - `--allow-dirty` allows worktree changes, staged changes, and
    //   untracked files.
    // - `--allow-staged` allows staged changes, but still rejects worktree
    //   changes and untracked files.
    if options.allow_no_vcs {
        eprintln!("--allow-no-vcs is set, skipping repository checks.");
        return Ok(());
    }

    let Some(repo) = Repository::discover(target_dir)? else {
        return Err("no VCS found for the target directory; if you'd like to suppress this error pass `--allow-no-vcs`".into());
    };

    let Some(changes) = repo.repository_changes()? else {
        return Ok(());
    };

    if options.allow_dirty {
        return Ok(());
    }

    if changes.has_dirty_files() {
        return Err(
            "the repository containing the target directory has uncommitted changes; if you'd like to suppress this error pass `--allow-dirty`".into(),
        );
    }

    if options.allow_staged {
        return Ok(());
    }

    if changes.has_staged_files() {
        return Err(
            "the repository containing the target directory has staged changes; if you'd like to suppress this error pass `--allow-staged`".into(),
        );
    }

    Ok(())
}
