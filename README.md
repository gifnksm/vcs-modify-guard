<!-- cargo-sync-rdme title [[ -->
# vcs-status
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
[![Maintenance: actively-developed](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg?style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-badges-section)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/vcs-status.svg?style=flat-square)](#license)
[![crates.io](https://img.shields.io/crates/v/vcs-status.svg?logo=rust&style=flat-square)](https://crates.io/crates/vcs-status)
[![docs.rs](https://img.shields.io/docsrs/vcs-status.svg?logo=docs.rs&style=flat-square)](https://docs.rs/vcs-status)
[![Rust: ^1.85.0](https://img.shields.io/badge/rust-^1.85.0-93450a.svg?logo=rust&style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
[![GitHub Actions: CI](https://img.shields.io/github/actions/workflow/status/gifnksm/vcs-status/ci.yml.svg?label=CI&logo=github&style=flat-square)](https://github.com/gifnksm/vcs-status/actions/workflows/ci.yml)
[![Codecov](https://img.shields.io/codecov/c/github/gifnksm/vcs-status.svg?label=codecov&logo=codecov&style=flat-square)](https://codecov.io/gh/gifnksm/vcs-status)
<!-- cargo-sync-rdme ]] -->

<!-- cargo-sync-rdme rustdoc [[ -->
A library for querying VCS working tree status.

## Usage

Add this to your `Cargo.toml`:

````toml
[dependencies]
vcs-status = "0.0.0"
````
<!-- cargo-sync-rdme ]] -->

## Minimum supported Rust version (MSRV)

The minimum supported Rust version is **Rust 1.85.0**.
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
