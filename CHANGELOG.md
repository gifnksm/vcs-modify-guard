# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Changed

* **Breaking:** Updated `gix` from 0.87.0 to 0.88.0. The `source` fields of `GixBackendError` variants now use `gix::Error` from 0.88.0; code relying on the 0.87.0 type must be updated.

## [0.1.1] - 2026-08-27

### Fixed

* Updated `gix` to fix build failures caused by the upstream `bisync` crate being yanked.

## [0.1.0] - 2026-08-02

* First release

<!-- next-url -->
[Unreleased]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/gifnksm/vcs-modify-guard/commits/v0.1.0
