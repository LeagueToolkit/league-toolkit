# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.15...ltk_wad-v0.3.0) - 2026-07-03

### Added

- *(wad)* add wad parsing tests and signature checks
- *(wad)* add plumbing for preserving wad signatures

### Other

- *(wad)* [**breaking**] rename builder setters to with_prebuilt_signature/checksum
- switch to per-crate changelogs
- update CI configuration and add linting settings for all crates

## [0.2.15](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.14...ltk_wad-v0.2.15) - 2026-04-07

### Other

- updated the following local packages: ltk_io_ext

## [0.2.14](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.13...ltk_wad-v0.2.14) - 2026-03-28

### Added

- *(wad)* add with_hash to WadChunkBuilder for custom file preservation

### Other

- Merge pull request #123 from DexalGT/feat/wad-chunk-builder-with-hash
- Update crates/ltk_wad/src/builder.rs
- Update crates/ltk_wad/src/builder.rs
- *(ltk_wad)* document with_path and with_hash on WadChunkBuilder

## [0.2.10](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.9...ltk_wad-v0.2.10) - 2026-01-14

### Other

- updated the following local packages: ltk_file

## [0.2.9](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.8...ltk_wad-v0.2.9) - 2025-12-27

### Other

- add new readme and llm docs/guide

## [0.2.8](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.7...ltk_wad-v0.2.8) - 2025-12-20

### Other

- updated the following local packages: ltk_io_ext

## [0.2.7](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.6...ltk_wad-v0.2.7) - 2025-12-12

### Other

- *(README)* add section on BC1/BC3 texture encoding with intel-tex feature

## [0.2.5](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.4...ltk_wad-v0.2.5) - 2025-11-27

### Other

- remove AGPL-3.0 license and update to dual licensing under MIT and Apache-2.0 for all crates; delete LICENSE file

## [0.2.4](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wad-v0.2.3...ltk_wad-v0.2.4) - 2025-11-26

### Other

- add badges to README
