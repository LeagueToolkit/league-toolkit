# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_rst-v0.1.0...ltk_rst-v0.2.0) - 2026-07-02

### Added

- *(ltk_rst)* [**breaking**] rework public API ([#138](https://github.com/LeagueToolkit/league-toolkit/pull/138))

## [0.1.0](https://github.com/LeagueToolkit/league-toolkit/releases/tag/ltk_rst-v0.1.0) - 2026-06-30

### Added

- *(rst)* [**breaking**] improved public API ([#136](https://github.com/LeagueToolkit/league-toolkit/pull/136))
- add ltk_rst crate for RST (Riot String Table) support

### Other

- *(ltk_rst)* run rustfmt
- *(ltk_rst)* address review feedback and simplify Stringtable API
- *(ltk_rst)* address PR review feedback
- fix rustfmt in parse_files tests
- *(ltk_rst)* add integration tests using real game files
- fix rustfmt struct literal in RstFile::from_reader
