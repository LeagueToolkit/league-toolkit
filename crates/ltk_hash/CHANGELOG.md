# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.3.0...ltk_hash-v0.4.0) - 2026-07-12

### Added

- *(ltk_texture)* add support for new formats and handle pixel formats

### Fixed

- *(hash)* From<&str> for {Wad,Bin}Hash

### Other

- *(ritobin)* [**breaking**] node indirection & other performance improvements ([#146](https://github.com/LeagueToolkit/league-toolkit/pull/146))
- switch to per-crate changelogs

## [0.3.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.6...ltk_hash-v0.3.0) - 2026-07-02

### Added

- *(hash)* [**breaking**] loads of hash type impls + rename 'from_str' -> 'hash_str' + serde flag
- *(hash)* add hash newtypes + remove hash functions from public api
- *(ltk_hash)* add xxhash64 re-export module

### Other

- Merge pull request [#127](https://github.com/LeagueToolkit/league-toolkit/pull/127) from LeagueToolkit/feat/ltk-hash-xxhash
- update CI configuration and add linting settings for all crates

## [0.2.6](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.5...ltk_hash-v0.2.6) - 2026-03-28

### Other

- add AI-assisted development workflow with Speckit to README

## [0.2.5](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.4...ltk_hash-v0.2.5) - 2025-12-27

### Other

- Merge pull request #92 from LeagueToolkit/better-docs
- add new readme and llm docs/guide

## [0.2.4](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.3...ltk_hash-v0.2.4) - 2025-12-12

### Other

- *(README)* add section on BC1/BC3 texture encoding with intel-tex feature

## [0.2.3](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.2...ltk_hash-v0.2.3) - 2025-11-27

### Other

- remove AGPL-3.0 license and update to dual licensing under MIT and Apache-2.0 for all crates; delete LICENSE file

## [0.2.2](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_hash-v0.2.1...ltk_hash-v0.2.2) - 2025-11-26

### Added

- add ltk_shader crate

### Other

- add badges to README
