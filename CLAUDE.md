# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Documentation

Four kinds of document, one job each. What separates them is not only subject but **lifecycle** — mixing a living document with a historical record is what turns a design doc into an unreadable pile:

| Document   | Answers                                                      | Tense      | Lifecycle                                   | Where                            |
| ---------- | ------------------------------------------------------------ | ---------- | ------------------------------------------- | -------------------------------- |
| **PRD**    | Why at all, for whom, what it must do (`FR-N`)               | Present    | Requirements append; numbers never shift    | `docs/prd/NNN-slug.md`           |
| **Spec**   | What is true: surface, wire format, traversal, errors, tests | Present    | **Edited in place, forever**                | `docs/design/<feature>.md`       |
| **ADR**    | Why this option and not that one                             | Past       | **Immutable** — superseded, never rewritten | `docs/adr/NNNN-slug.md`          |
| **Ticket** | What to build next                                           | Imperative | Closed when done                            | `.scratch/<project>/issues/*.md` |

- **Every rule has exactly one home, and it is the spec.** An ADR records that a choice was made and what it beat; it is never where a reader looks to learn how the code behaves today. A rule stated in two places has one stale copy and no way to tell which.
- **Domain vocabulary lives in the spec**, in a section near the top. A definition changes as the domain is understood better, so it needs a container that can be rewritten — an immutable dated record is the wrong one.
- **A spec is edited in place, never appended to.** When the code departs from it, the section that stated the old thing is rewritten to state the new one. No phase sections, no "implementation notes", no correction notes appended below. Measurements are the one exception: they are facts about a specific build and live in a dated appendix.
- A spec **cites**: requirements as `FR-N`, decisions as `ADR-NNNN`. It does not restate them. Two copies of one argument drift.
- **A section reference is a linked "section N".** Every numbered heading in a PRD or a spec carries a stable anchor — `## <a id="s4.3"></a>4.3. Views` — and every citation is a link whose text says what it points at: `[section 4.3](#s4.3)`, parenthesised where it is an aside, and spelled out in full on both halves of a pair or a range — `[section 4.2](#s4.2) and [section 4.3](#s4.3)`, `[section 4](#s4) to [section 9](#s9)` — so no link ever renders as a bare number with nothing to say what it is. Appendices are `[appendix B](#appendix-b)`. A cross-document reference names the file first — `` `ptch-property-patches.md` [section 6](ptch-property-patches.md#s6) `` — and a ticket uses the absolute `https://github.com/LeagueToolkit/league-toolkit/blob/main/<path>#sN` form, because a bare fragment in an issue body resolves against the issue page instead. Inside a code block a reference stays plain prose: a link cannot render there, and doc comments get copied into source. The anchor is the point — a heading can be reworded freely and no citation breaks.
- Write an ADR before adding a crate, changing the shape of a public API, or diverging from what the game client does. Name at least two viable alternatives with concrete trade-offs.
- Templates: `docs/prd/template.md`, `docs/adr/template.md`. Worked example: PRD-001, ADR-0001 to ADR-0006 and `docs/design/ptch-property-patches.md`.
- Skills: `write-prd`, `write-spec`, `write-adr` and `write-ticket` produce these files, `sync-issues` renders tickets to GitHub. Each carries the rule for its own document (when it is worth writing, how it is numbered, what it must not absorb).

## Issue Sync

GitHub issues are rendered from the ticket files in `.scratch/*/issues/` (frontmatter `issue: N` maps each ticket to its issue). When a task changes a ticket file, or a document under `docs/design/`, `docs/prd/` or `docs/adr/` that a ticket renders from, run the `sync-issues` skill before finishing so the issues never drift from the repo. Anything published to GitHub (issues, PR bodies, commits) is written in the maintainer's voice — no AI attribution of any kind.

## Project Overview

League Toolkit is a Rust workspace for parsing, editing, and writing League of Legends file formats. It consists of 13 individual `ltk_*` crates plus one umbrella `league-toolkit` crate that re-exports them via feature flags.

## Build Commands

```bash
cargo build --verbose          # Build all crates
cargo test --verbose           # Run all tests
cargo fmt -- --check           # Check formatting
cargo clippy --all-targets     # Lint (severity controlled by [workspace.lints] in root Cargo.toml)
```

Run a single crate's tests:

```bash
cargo test -p ltk_meta --verbose
```

Run a specific test:

```bash
cargo test -p ltk_meta test_name
```

Snapshot tests use `cargo-insta`. To review snapshot changes:

```bash
cargo insta review
```

## Workspace Structure

All crates live under `crates/`. The dependency graph flows upward:

- **Foundation**: `ltk_hash`, `ltk_primitives` (no internal deps)
- **I/O layer**: `ltk_io_ext` (depends on `ltk_primitives`)
- **Format crates**: `ltk_wad`, `ltk_texture`, `ltk_mesh`, `ltk_anim`, `ltk_meta`, `ltk_file` (depend on foundation + I/O)
- **Higher-level**: `ltk_mapgeo` (depends on `ltk_mesh`), `ltk_ritobin` (depends on `ltk_meta`), `ltk_shader` (depends on `ltk_wad`)
- **Umbrella**: `league-toolkit` re-exports everything behind feature flags

## Key Patterns

**Reading/Writing**: Most types implement `from_reader(&mut impl Read)` and `to_writer(&mut impl Write)`. WAD mounting requires `Read + Seek`.

**Builder pattern**: Complex types use builders — `BinTree::builder()`, `BinTreeObject::builder()`, `RigResource::builder()`, `WadBuilder`.

**Error handling**: Each crate defines its own error type via `thiserror` and a `Result<T>` type alias. `ltk_meta` additionally uses `miette` for diagnostic errors.

**Math**: All vector/matrix types use `glam` (Vec2, Vec3, Vec4, Mat4, Quat).

**Hashing**: WAD paths are XXHash64 (64-bit) of lowercased paths. Bin object/property names are FNV-1a (32-bit) hashes via `ltk_hash::fnv1a::hash_lower()`.

## Crate Layout Convention

Each crate typically follows:

```tree
crates/ltk_*/
|-- src/
|   |-- lib.rs        # Re-exports + module declarations
|   |-- error.rs      # Error enum (thiserror)
|   |-- ...           # Type modules
|-- tests/            # Integration tests (some crates)
|-- Cargo.toml
```

Snapshot test data lives in `crates/*/src/**/snapshots/` or `crates/*/tests/snapshots/`.

## Testing Approach

- Unit tests inline in source files
- Integration tests in `crates/*/tests/` (ltk_anim, ltk_meta, ltk_mapgeo, ltk_ritobin)
- Round-trip tests (parse → write → parse → assert equal) are the primary verification pattern
- Snapshot tests use `insta` with `.ron` format
- `approx` crate for floating-point comparisons

## Workspace Dependencies

Shared dependency versions are declared in the root `Cargo.toml` under `[workspace.dependencies]`. Individual crates reference them with `workspace = true`. When adding dependencies, prefer adding them at workspace level.

## Engineering Rules

- **Crate boundaries.** Each format parser lives in its own `ltk_*` crate, compilable and usable without the umbrella. The dependency graph flows upward and never cycles. `league-toolkit` re-exports behind feature flags and holds no logic of its own.
- **Round-trip or read-only.** A type that reads a format also writes it, or the crate documents the format as read-only. The round-trip test is the primary verification; `approx` for floats, `insta` with `.ron` for snapshots, reviewed with `cargo insta review` before committing.
- **The CI gate is all three.** `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` come back clean before a merge to `main`. Suppress a lint with `#[expect]` and a reason, never a bare `#[allow]`.
- **No `unwrap`/`expect` in library code.** Propagate with `?` or add an error variant. A panic is for a bug, and it states what failed and with what value. Every `unsafe` block carries a `// SAFETY:` comment naming the invariant that makes it sound.
- **Dependencies land at workspace level** in `[workspace.dependencies]`, referenced with `workspace = true`. A crate-local version pin needs a stated reason, and a new dependency needs a justification proportional to the tree it drags in.
- **Hash algorithms are fixed by the game.** WAD paths are XXHash64 of the lowercased path; bin names are FNV-1a 32-bit. Changing either needs format evidence, not a preference.
- **Work lands through PRs** on feature branches, with conventional commit subjects (`feat(ltk_wad):`, `fix(ltk_meta):`). A branch is rebased onto `main` and force-pushed — never have `main` merged into it.

## Additional Context

The `docs/LTK_GUIDE.md` file contains detailed crate-by-crate API documentation with usage examples, file format references, and hash algorithm details. Consult it for format-specific questions.
