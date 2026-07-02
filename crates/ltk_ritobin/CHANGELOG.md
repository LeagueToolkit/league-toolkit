# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.3.0...ltk_ritobin-v0.4.0) - 2026-07-02

### Added

- *(ritobin)* [**breaking**] use new hash types

### Fixed

- *(ritobin)* properly handle inline comments ([#134](https://github.com/LeagueToolkit/league-toolkit/pull/134))

### Other

- Merge pull request [#128](https://github.com/LeagueToolkit/league-toolkit/pull/128) from LeagueToolkit/ritobin-readme
- *(ltk_ritobin)* add readme docs
- update CI configuration and add linting settings for all crates

## [0.2.2](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.2.1...ltk_ritobin-v0.2.2) - 2026-03-28

### Other

- add AI-assisted development workflow with Speckit to README

## [0.2.1](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.2.0...ltk_ritobin-v0.2.1) - 2026-03-03

### Fixed

- *(ltk_ritobin)* transpose Mat4 for correct row-major text output

### Other

- rustfmt
- *(ltk_ritobin)* add mtx44 round-trip test and inline comments

## [0.1.6](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.1.5...ltk_ritobin-v0.1.6) - 2026-02-01

### Other

- updated the following local packages: ltk_primitives, ltk_meta

## [0.1.5](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.1.4...ltk_ritobin-v0.1.5) - 2025-12-27

### Other

- Merge pull request #92 from LeagueToolkit/better-docs
- add new readme and llm docs/guide

## [0.1.4](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.1.3...ltk_ritobin-v0.1.4) - 2025-12-20

### Other

- updated the following local packages: ltk_primitives, ltk_meta

## [0.1.2](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_ritobin-v0.1.1...ltk_ritobin-v0.1.2) - 2025-12-12

### Other

- *(README)* add section on BC1/BC3 texture encoding with intel-tex feature

## [0.1.0](https://github.com/LeagueToolkit/league-toolkit/releases/tag/ltk_ritobin-v0.1.0) - 2025-12-11

### Added

- support hashtable provider during ritobin writing
- implement nom_locate to support miette
- add vibecoded ritobin parser

### Fixed

- suppress unused assignment warnings in error module and clean up imports in writer module

### Other

- release
- remove AGPL-3.0 license and update to dual licensing under MIT and Apache-2.0 for all crates; delete LICENSE file
- add badges to README
- update licensing
- update README
- Update README.md ([#21](https://github.com/LeagueToolkit/league-toolkit/pull/21))
- Initial commit
