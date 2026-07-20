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
//! use std::path::{Path, PathBuf};
//!
//! use clap::Parser;
//! use vcs_status::{AllowOptions, CheckResult};
//!
//! #[derive(Debug, Parser)]
//! struct Args {
//!     /// Process code even if a VCS was not detected.
//!     #[arg(long)]
//!     allow_no_vcs: bool,
//!     /// Process code even if the target directory has modified, staged, or
//!     /// untracked files under it.
//!     #[arg(long)]
//!     allow_dirty: bool,
//!     /// Process code even if the target directory has staged changes under it.
//!     #[arg(long)]
//!     allow_staged: bool,
//!     /// Target directory to process. Defaults to the current working directory.
//!     /// Only this directory is checked by default.
//!     #[arg(long)]
//!     target_dir: Option<PathBuf>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let args = Args::parse();
//!
//!     let target_dir = args.target_dir.as_deref().unwrap_or_else(|| Path::new("."));
//!     let result = AllowOptions::new()
//!         .allow_no_vcs(args.allow_no_vcs)
//!         .allow_dirty(args.allow_dirty)
//!         .allow_staged(args.allow_staged)
//!         .check_safe_to_modify(target_dir)?;
//!
//!     match result {
//!         CheckResult::Allowed => {}
//!         CheckResult::BlockedByNoVcs => {
//!             return Err("blocked by no VCS".into());
//!         }
//!         CheckResult::BlockedByDirty { .. } => {
//!             return Err("blocked by dirty files".into());
//!         }
//!         CheckResult::BlockedByStaged { .. } => {
//!             return Err("blocked by staged changes".into());
//!         }
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
pub use self::{allow_options::*, error::*, repository::Repository};

mod allow_options;
mod error;
pub mod repository;
#[cfg(test)]
mod testing;
mod util;
mod vcs;
