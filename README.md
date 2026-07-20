<!-- cargo-sync-rdme title [[ -->
# vcs-status
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
[![Maintenance: actively-developed](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg?style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-badges-section)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/vcs-status.svg?style=flat-square)](#license)
[![crates.io](https://img.shields.io/crates/v/vcs-status.svg?logo=rust&style=flat-square)](https://crates.io/crates/vcs-status)
[![docs.rs](https://img.shields.io/docsrs/vcs-status.svg?logo=docs.rs&style=flat-square)](https://docs.rs/vcs-status)
[![Rust: ^1.96.0](https://img.shields.io/badge/rust-^1.96.0-93450a.svg?logo=rust&style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
[![GitHub Actions: CI](https://img.shields.io/github/actions/workflow/status/gifnksm/vcs-status/ci.yml.svg?label=CI&logo=github&style=flat-square)](https://github.com/gifnksm/vcs-status/actions/workflows/ci.yml)
[![Codecov](https://img.shields.io/codecov/c/github/gifnksm/vcs-status.svg?label=codecov&logo=codecov&style=flat-square)](https://codecov.io/gh/gifnksm/vcs-status)
<!-- cargo-sync-rdme ]] -->

<!-- cargo-sync-rdme rustdoc [[ -->
Help CLI tools decide whether it is safe to modify files in a VCS
working tree.

`vcs-status` provides a small abstraction over version control systems
for checking whether a working tree has modified, staged, or untracked
files. It is intended for CLI tools that implement options such as
`--allow-dirty`, `--allow-staged`, and `--allow-no-vcs`.

## Example

The following example shows how to validate the changes in a repository
before performing an operation that may modify files.

````rust,no_run
use std::path::{Path, PathBuf};

use clap::Parser;
use vcs_status::{AllowOptions, CheckResult};

#[derive(Debug, Parser)]
struct Args {
    /// Process code even if a VCS was not detected.
    #[arg(long)]
    allow_no_vcs: bool,
    /// Process code even if the target directory has modified, staged, or
    /// untracked files under it.
    #[arg(long)]
    allow_dirty: bool,
    /// Process code even if the target directory has staged changes under it.
    #[arg(long)]
    allow_staged: bool,
    /// Target directory to process. Defaults to the current working directory.
    /// Only this directory is checked by default.
    #[arg(long)]
    target_dir: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let target_dir = args.target_dir.as_deref().unwrap_or_else(|| Path::new("."));
    let result = AllowOptions::new()
        .allow_no_vcs(args.allow_no_vcs)
        .allow_dirty(args.allow_dirty)
        .allow_staged(args.allow_staged)
        .check_safe_to_modify(target_dir)?;

    match result {
        CheckResult::Allowed => {}
        CheckResult::BlockedByNoVcs => {
            return Err("blocked by no VCS".into());
        }
        CheckResult::BlockedByDirty { .. } => {
            return Err("blocked by dirty files".into());
        }
        CheckResult::BlockedByStaged { .. } => {
            return Err("blocked by staged changes".into());
        }
    }

    eprintln!("Proceeding...");

    Ok(())
}
````

See the `allow_options` example for a complete command-line application.

## Usage

Add this to your `Cargo.toml`:

````toml
[dependencies]
vcs-status = "0.1.0"
````
<!-- cargo-sync-rdme ]] -->

## Minimum supported Rust version (MSRV)

The minimum supported Rust version is **Rust 1.96.0**.
At least the last 3 versions of stable Rust are supported at any given time.

While a crate is a pre-release status (0.x.x) it may have its MSRV bumped in a patch release.
Once a crate has reached 1.x, any MSRV bump will be accompanied by a new minor version.

## License

This project is licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
