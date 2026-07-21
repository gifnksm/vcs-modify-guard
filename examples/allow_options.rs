//! Demonstrates path-scoped `--allow-dirty`, `--allow-staged`, and
//! `--allow-no-vcs` checks using [`vcs_modify_guard::AllowOptions`].
//!
//! This example uses the same `--allow-*` flag semantics as `cargo fix`, but
//! applies them only to the queried path by default.

use std::path::{Path, PathBuf};

use clap::Parser;
use vcs_modify_guard::{AllowOptions, CheckResult};

#[derive(Debug, Parser)]
struct Args {
    /// Process code even if a VCS was not detected.
    #[arg(long)]
    allow_no_vcs: bool,
    /// Process code even if the target path has modified, staged, or
    /// untracked files under it.
    #[arg(long)]
    allow_dirty: bool,
    /// Process code even if the target path has staged changes under it.
    #[arg(long)]
    allow_staged: bool,
    /// Target path to process. Defaults to the current working directory.
    target: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let target = args.target.as_deref().unwrap_or_else(|| Path::new("."));
    let result = AllowOptions::new()
        .allow_no_vcs(args.allow_no_vcs)
        .allow_dirty(args.allow_dirty)
        .allow_staged(args.allow_staged)
        .check_safe_to_modify(target)?;

    match result {
        CheckResult::Allowed => {}
        CheckResult::BlockedByNoVcs => {
            eprintln!(
                "The target path is not in a VCS repository. Use `--allow-no-vcs` to override this check."
            );
            return Err("blocked by no VCS".into());
        }
        CheckResult::BlockedByDirty {
            worktree,
            dirty_files,
            staged_files,
        } => {
            eprintln!(
                "The target path has uncommitted changes under it. Use `--allow-dirty` to override this check."
            );
            eprintln!("Worktree: {}", worktree.display());
            for file in dirty_files {
                eprintln!("* {} (dirty)", file.display());
            }
            for file in staged_files {
                eprintln!("* {} (staged)", file.display());
            }
            return Err("blocked by dirty files".into());
        }
        CheckResult::BlockedByStaged {
            worktree,
            staged_files,
        } => {
            eprintln!(
                "The target path has staged changes under it. Use `--allow-staged` to override this check."
            );
            eprintln!("Worktree: {}", worktree.display());
            for file in staged_files {
                eprintln!("* {} (staged)", file.display());
            }
            return Err("blocked by staged changes".into());
        }
    }

    eprintln!("Proceeding...");

    Ok(())
}
