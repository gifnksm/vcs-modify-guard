//! Help CLI tools decide whether it is safe to modify files in a VCS
//! working tree.
//!
//! `vcs-modify-guard` helps CLI tools enforce `--allow-dirty`,
//! `--allow-staged`, and `--allow-no-vcs` style checks before they modify
//! files.
//!
//! This crate provides two layers of API:
//!
//! - [`AllowOptions`] is the main entry point. It implements `cargo fix`-style
//!   safe-to-modify checks and returns a [`ModificationSafety`] describing whether
//!   modification is safe. By default, checks are scoped to the queried path.
//! - [`repository::Repository`] is a lower-level API for tools that need to
//!   discover a repository and inspect whether files are dirty and/or staged to
//!   implement their own policy. Dirty files include modified tracked files and
//!   untracked files.
//!
//! Most users should start with [`AllowOptions`]. Reach for
//! [`repository::Repository`] only when you need custom behavior beyond the
//! built-in `--allow-*` semantics.
//!
//! # Example
//!
//! The following example shows how to validate whether a target path is safe
//! to modify before performing an operation that may modify files.
//!
//! ```no_run
//! use std::path::{Path, PathBuf};
//!
//! use clap::Parser;
//! use vcs_modify_guard::{AllowOptions, ModificationSafety, UnsafeModificationReason};
//!
//! #[derive(Debug, Parser)]
//! struct Args {
//!     /// Process code even if a VCS was not detected.
//!     #[arg(long)]
//!     allow_no_vcs: bool,
//!     /// Process code even if the target path has modified, staged, or
//!     /// untracked files under it.
//!     #[arg(long)]
//!     allow_dirty: bool,
//!     /// Process code even if the target path has staged changes under it.
//!     #[arg(long)]
//!     allow_staged: bool,
//!     /// Target path to process. Defaults to the current working directory.
//!     target: Option<PathBuf>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let args = Args::parse();
//!
//!     let target = args.target.as_deref().unwrap_or_else(|| Path::new("."));
//!     let safety = AllowOptions::new()
//!         .allow_no_vcs(args.allow_no_vcs)
//!         .allow_dirty(args.allow_dirty)
//!         .allow_staged(args.allow_staged)
//!         .check_safe_to_modify(target)?;
//!
//!     match safety {
//!         ModificationSafety::Safe => {}
//!         ModificationSafety::Unsafe(reason) => match reason {
//!             UnsafeModificationReason::NoVcs => {
//!                 return Err("blocked by no VCS".into());
//!             }
//!             UnsafeModificationReason::Dirty { .. } => {
//!                 return Err("blocked by dirty files".into());
//!             }
//!             UnsafeModificationReason::Staged { .. } => {
//!                 return Err("blocked by staged changes".into());
//!             }
//!         },
//!     }
//!
//!     eprintln!("Proceeding...");
//!
//!     Ok(())
//! }
//! ```
//!
//! See the `allow_options` example for a complete command-line application.
//!
//! If you need custom policy logic instead of the built-in `--allow-*`
//! behavior, see the [`repository`] module for direct repository discovery and
//! change query APIs.
//!
//! # Usage
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! vcs-modify-guard = "0.1.0"
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_root_url = "https://docs.rs/vcs-modify-guard/0.1.0")]

#[cfg_attr(
    not(vcs_backend_enabled),
    expect(
        unused_imports,
        unreachable_pub,
        reason = "when no VCS backend is enabled, `vcs::*` re-exports nothing from the crate root"
    )
)]
pub use self::vcs::*;
pub use self::{allow_options::*, error::*};

mod allow_options;
mod error;
pub mod repository;
#[cfg(test)]
mod testing;
mod util;
mod vcs;
