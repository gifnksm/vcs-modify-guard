# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Changed

* **Breaking:** Updated the `gix` dependency from 0.90.0 to 0.91.0. The
  `git-gix` feature enables this dependency to provide the Git backend. The
  `source` fields of `GixBackendError` variants now use `gix::Error` as exported
  by `gix` 0.91.0. If you also depend on `gix` directly and work with these
  errors, update to 0.91.0 for the matching API.

### Security

* Updating the `gix` backend removes its transitive dependency on `heapless`
  0.8.0 from `Cargo.lock`, clearing the audit findings for [RUSTSEC-2026-0574],
  [RUSTSEC-2026-0575], [RUSTSEC-2026-0576], [RUSTSEC-2026-0577],
  [RUSTSEC-2026-0578], [RUSTSEC-2026-0579], [RUSTSEC-2026-0580],
  [RUSTSEC-2026-0581], and [RUSTSEC-2026-0582].

  In `vcs-modify-guard` 0.3.0, `heapless` was an unused transitive dependency of
  the Git backend, so these advisories did not affect the backend.

[RUSTSEC-2026-0574]: https://rustsec.org/advisories/RUSTSEC-2026-0574.html
[RUSTSEC-2026-0575]: https://rustsec.org/advisories/RUSTSEC-2026-0575.html
[RUSTSEC-2026-0576]: https://rustsec.org/advisories/RUSTSEC-2026-0576.html
[RUSTSEC-2026-0577]: https://rustsec.org/advisories/RUSTSEC-2026-0577.html
[RUSTSEC-2026-0578]: https://rustsec.org/advisories/RUSTSEC-2026-0578.html
[RUSTSEC-2026-0579]: https://rustsec.org/advisories/RUSTSEC-2026-0579.html
[RUSTSEC-2026-0580]: https://rustsec.org/advisories/RUSTSEC-2026-0580.html
[RUSTSEC-2026-0581]: https://rustsec.org/advisories/RUSTSEC-2026-0581.html
[RUSTSEC-2026-0582]: https://rustsec.org/advisories/RUSTSEC-2026-0582.html

## [0.3.0] - 2026-10-24

### Changed

* **Breaking:** Updated `gix` from 0.89.0 to 0.90.0. The `source` fields of
  `GixBackendError` variants now use `gix::Error` from 0.90.0; code relying on
  the 0.89.0 type must be updated.

### Security

* Updated `gix` to fix [GHSA-q54r-jf2p-rfr2]. On some Windows and macOS
  filesystems, an attacker-controlled repository could cause `gix` to overwrite
  Git's own settings or hooks when writing files, for example during checkout or
  merge. This crate only checks for file changes and does not perform these
  writes.

[GHSA-q54r-jf2p-rfr2]: https://github.com/advisories/GHSA-q54r-jf2p-rfr2

## [0.2.0] - 2026-10-13

### Changed

* **Breaking:** Updated `gix` from 0.87.0 to 0.89.0. The `source` fields of
  `GixBackendError` variants now use `gix::Error` from 0.89.0; code relying on
  the 0.87.0 type must be updated.

### Added

* Added `GixBackendError::EncodePath` and `GixBackendError::DecodePath` to
  report failures when converting between filesystem paths and Git path bytes.

## [0.1.1] - 2026-08-27

### Fixed

* Updated `gix` to fix build failures caused by the upstream `bisync` crate
  being yanked.

## [0.1.0] - 2026-08-02

* First release

<!-- next-url -->
[Unreleased]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/gifnksm/vcs-modify-guard/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/gifnksm/vcs-modify-guard/commits/v0.1.0
