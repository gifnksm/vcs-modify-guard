//! Demonstrates how a CLI tool can implement `--allow-dirty`,
//! `--allow-staged`, and `--allow-no-vcs` options with behavior
//! equivalent to `cargo fix`.

use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Parser;
use vcs_status::Repository;

#[derive(Debug, Parser)]
struct Args {
    /// Process code even if a VCS was not detected.
    #[arg(long)]
    allow_no_vcs: bool,

    /// Process code even if the working directory is dirty or has staged changes.
    #[arg(long)]
    allow_dirty: bool,

    /// Process code even if the working directory has staged changes.
    #[arg(long)]
    allow_staged: bool,

    /// Target directory to process. Defaults to the current working directory.
    #[arg(long)]
    target_dir: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let target_dir = args.target_dir.as_deref().unwrap_or_else(|| Path::new("."));
    let options = AllowOptions {
        allow_no_vcs: args.allow_no_vcs,
        allow_staged: args.allow_staged,
        allow_dirty: args.allow_dirty,
    };

    ensure_repository_status(target_dir, &options)?;

    eprintln!("Proceeding...");

    Ok(())
}

struct AllowOptions {
    allow_no_vcs: bool,
    allow_dirty: bool,
    allow_staged: bool,
}

fn ensure_repository_status(
    target_dir: &Path,
    options: &AllowOptions,
) -> Result<(), Box<dyn Error>> {
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
        return Err("no VCS found for the target directory and this command can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-no-vcs`".into());
    };

    let status = repo.status()?;

    if options.allow_dirty {
        return Ok(());
    }

    if status.has_worktree_changes() || status.has_untracked_files() {
        return Err(
            "the target directory has uncommitted changes, and this command can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-dirty`".into(),
        );
    }

    if options.allow_staged {
        return Ok(());
    }

    if status.has_staged_changes() {
        return Err(
            "the target directory has staged changes, and this command can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-staged`".into(),
        );
    }

    Ok(())
}
