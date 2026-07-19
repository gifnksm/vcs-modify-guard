//! Help CLI tools decide whether it is safe to modify files in a VCS
//! working tree.
//!
//! `vcs-status` provides a small abstraction over version control systems
//! for checking whether a working tree has modified, staged, or untracked
//! files. It is intended for CLI tools that implement options such as
//! `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs`.
//!
//! # Example
//!
//! The following example shows how to validate the changes in a repository
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
//! fn ensure_safe_to_modify(
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
//!         return Err("no VCS found for the target directory".into());
//!     };
//!
//!     let Some(changes) = repo.repository_changes()? else {
//!        return Ok(());
//!     };
//!
//!     if options.allow_dirty {
//!         return Ok(());
//!     }
//!
//!     if changes.has_modified_files() || changes.has_untracked_files() {
//!         return Err(
//!             "the repository containing the target directory has uncommitted changes"
//!                 .into(),
//!         );
//!     }
//!
//!     if options.allow_staged {
//!         return Ok(());
//!     }
//!
//!     if changes.has_staged_files() {
//!         return Err(
//!             "the repository containing the target directory has staged changes"
//!                 .into(),
//!         );
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

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_root_url = "https://docs.rs/vcs-status/0.1.0")]

#[cfg_attr(
    not(vcs_backend_enabled),
    expect(
        unused_imports,
        unreachable_pub,
        reason = "when no VCS backend is enabled, `vcs::*` re-exports nothing from the crate root"
    )
)]
pub use self::vcs::*;
pub use self::{error::*, repository::Repository};

mod error;
pub mod repository;
#[cfg(test)]
mod testing;
mod util;
mod vcs;
