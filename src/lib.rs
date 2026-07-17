//! Query the status of a VCS working tree.
//!
//! `vcs-status` provides a small abstraction over version control systems
//! for checking whether a working tree contains worktree changes, staged
//! changes, or untracked files.
//!
//! It is intended for CLI tools that implement options such as
//! `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs`.
//!
//! # Example
//!
//! The following example shows how to validate the status of a repository
//! before performing an operation that may modify files.
//!
//! ```no_run
//! use std::{error::Error, path::Path};
//!
//! use vcs_status::Repository;
//!
//! struct AllowOptions {
//!     allow_no_vcs: bool,
//!     allow_dirty: bool,
//!     allow_staged: bool,
//! }
//!
//! fn ensure_repository_status(
//!     target_dir: &Path,
//!     options: &AllowOptions,
//! ) -> Result<(), Box<dyn Error>> {
//!     // Match `cargo fix` exactly:
//!     // - `--allow-no-vcs` allows running even when no repository is found.
//!     // - `--allow-dirty` allows worktree changes, staged changes, and
//!     //   untracked files.
//!     // - `--allow-staged` allows staged changes, but still rejects
//!     //   worktree changes and untracked files.
//!     if options.allow_no_vcs {
//!         return Ok(());
//!     }
//!
//!     let Some(repo) = Repository::discover(target_dir)? else {
//!         return Err("not inside a VCS repository".into());
//!     };
//!
//!     let status = repo.status()?;
//!
//!     if options.allow_dirty {
//!         return Ok(());
//!     }
//!
//!     if status.has_worktree_changes() || status.has_untracked_files() {
//!         return Err("repository has uncommitted changes".into());
//!     }
//!
//!     if options.allow_staged {
//!         return Ok(());
//!     }
//!
//!     if status.has_staged_changes() {
//!         return Err("repository has staged changes".into());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! See the `allow_options` example for a complete command-line application.
//!
//! # Usage
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! vcs-status = "0.1.0"
//! ```

#![doc(html_root_url = "https://docs.rs/vcs-status/0.1.0")]

pub use self::{error::VcsStatusError, repository::*};

mod error;
mod repository;
mod vcs;
