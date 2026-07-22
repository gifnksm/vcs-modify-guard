<!-- cargo-sync-rdme title [[ -->
# vcs-modify-guard
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
[![Maintenance: actively-developed](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg?style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-badges-section)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/vcs-modify-guard.svg?style=flat-square)](#license)
[![crates.io](https://img.shields.io/crates/v/vcs-modify-guard.svg?logo=rust&style=flat-square)](https://crates.io/crates/vcs-modify-guard)
[![docs.rs](https://img.shields.io/docsrs/vcs-modify-guard.svg?logo=docs.rs&style=flat-square)](https://docs.rs/vcs-modify-guard)
[![Rust: ^1.96.0](https://img.shields.io/badge/rust-^1.96.0-93450a.svg?logo=rust&style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
[![GitHub Actions: CI](https://img.shields.io/github/actions/workflow/status/gifnksm/vcs-modify-guard/ci.yml.svg?label=CI&logo=github&style=flat-square)](https://github.com/gifnksm/vcs-modify-guard/actions/workflows/ci.yml)
[![Codecov](https://img.shields.io/codecov/c/github/gifnksm/vcs-modify-guard.svg?label=codecov&logo=codecov&style=flat-square)](https://codecov.io/gh/gifnksm/vcs-modify-guard)
<!-- cargo-sync-rdme ]] -->

<!-- cargo-sync-rdme rustdoc [[ -->
Help CLI tools decide whether it is safe to modify files in a VCS
working tree.

`vcs-modify-guard` helps CLI tools enforce `--allow-dirty`,
`--allow-staged`, and `--allow-no-vcs` style checks before they modify
files.

This crate provides two layers of API:

* [`AllowOptions`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/allow_options/struct.AllowOptions.html) is the main entry point. It implements `cargo fix`-style
  safe-to-modify checks and returns a [`ModificationSafety`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/allow_options/enum.ModificationSafety.html) describing whether
  modification is safe. By default, checks are scoped to the queried path.
* [`repository::Repository`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/repository/struct.Repository.html) is a lower-level API for tools that need to
  discover a repository and inspect whether files are dirty and/or staged to
  implement their own policy. Dirty files include modified tracked files and
  untracked files.

Most users should start with [`AllowOptions`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/allow_options/struct.AllowOptions.html). Reach for
[`repository::Repository`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/repository/struct.Repository.html) only when you need custom behavior beyond the
built-in `--allow-*` semantics.

## Example

The following example shows how to validate whether a target path is safe
to modify before performing an operation that may modify files.

````rust,no_run
use std::path::{Path, PathBuf};

use clap::Parser;
use vcs_modify_guard::{AllowOptions, ModificationSafety, UnsafeModificationReason};

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
    let safety = AllowOptions::new()
        .allow_no_vcs(args.allow_no_vcs)
        .allow_dirty(args.allow_dirty)
        .allow_staged(args.allow_staged)
        .check_safe_to_modify(target)?;

    match safety {
        ModificationSafety::Safe => {}
        ModificationSafety::Unsafe(reason) => match reason {
            UnsafeModificationReason::NoVcs => {
                return Err("blocked by no VCS".into());
            }
            UnsafeModificationReason::Dirty { .. } => {
                return Err("blocked by dirty files".into());
            }
            UnsafeModificationReason::Staged { .. } => {
                return Err("blocked by staged changes".into());
            }
        },
    }

    eprintln!("Proceeding...");

    Ok(())
}
````

See the `allow_options` example for a complete command-line application.

If you need custom policy logic instead of the built-in `--allow-*`
behavior, see the [`repository`](https://docs.rs/vcs-modify-guard/0.1.0/vcs_modify_guard/repository/index.html) module for direct repository discovery and
change query APIs.

## Usage

Add this to your `Cargo.toml`:

````toml
[dependencies]
vcs-modify-guard = "0.1.0"
````
<!-- cargo-sync-rdme ]] -->

## Minimum supported Rust version (MSRV)

The minimum supported Rust version is **Rust 1.96.0**.
At least the last 3 versions of stable Rust are supported at any given time.

While a crate is a pre-release status (0.x.x) it may have its MSRV bumped in a patch release.
Once a crate has reached 1.x, any MSRV bump will be accompanied by a new minor version.

## License

This project is licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
