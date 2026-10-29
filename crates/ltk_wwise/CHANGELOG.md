# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/LeagueToolkit/league-toolkit/compare/ltk_wwise-v0.1.0...ltk_wwise-v0.2.0) - 2026-10-29

### Added

- *(ltk_wwise)* decode props, containers and event trees

## [0.1.0](https://github.com/LeagueToolkit/league-toolkit/releases/tag/ltk_wwise-v0.1.0) - 2026-10-28

### Added

- *(ltk_wwise)* upgrade legacy banks to version 145
- *(ltk_wwise)* resolve event sources
- *(ltk_wwise)* decode and encode typed objects
- *(ltk_wwise)* read and write hierarchy framing
- *(ltk_wwise)* read and write banks and media
- *(ltk_wwise)* read, write and mount wpk packages
- *(ltk_wwise)* read wem format
- *(ltk_wwise)* classify audio paths
- *(ltk_wwise)* add crate and fnv1 hash
- *(ltk_ini)* add inibin reader and writer
- *(ltk_mapgeo)* add nvr reader
- *(ltk_texture)* add support for new formats and handle pixel formats

### Fixed

- *(ltk_wwise)* stream package media on read
- *(ltk_wwise)* skip non-play actions in event sources
- *(ltk_wwise)* refuse upgrade of raw chunks
- *(ltk_wwise)* align data in written banks
- *(ltk_wwise)* lay out media that miss the bank alignment
- *(ltk_wwise)* compare media indexes by content
- *(ltk_wwise)* display values in error messages
- *(ltk_wwise)* display object kinds in camel case

### Other

- *(ltk_wwise)* share u32 field conversion and package reader
- *(ltk_wwise)* add upgrade corpus test
- *(ltk_wwise)* add typed corpus test
- *(ltk_wwise)* add hierarchy corpus test
- *(ltk_wwise)* add bank corpus test
- *(ltk_wwise)* read wem format of shipped packages
- *(ltk_wwise)* add package fixture
- *(ltk_wwise)* add bank fixtures and corpus helper
- relicense under Apache-2.0
- link ritobin-lang
- rewrite workspace readme
- rework workflow
- *(meta)* crate-scoped README with the public surface and examples
- add AI-assisted development workflow with Speckit to README
- add new readme and llm docs/guide
- *(README)* add section on BC1/BC3 texture encoding with intel-tex feature
- remove AGPL-3.0 license and update to dual licensing under MIT and Apache-2.0 for all crates; delete LICENSE file
- add badges to README
- update licensing
- update README
- Update README.md ([#21](https://github.com/LeagueToolkit/league-toolkit/pull/21))
- Initial commit
