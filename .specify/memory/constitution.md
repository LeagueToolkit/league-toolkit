<!--
  Sync Impact Report
  ==================
  Version change: 1.0.0 -> 2.0.0 (MAJOR — full redefinition of all principles)
  Modified principles:
    - "I. Code Quality & Conventions" -> "I. Crate-First Architecture"
    - "II. Testing Standards" -> "II. Round-Trip Correctness"
    - "III. User Experience Consistency" -> "III. Strict CI Quality Gate"
    - "IV. Performance Requirements" -> "IV. Idiomatic Rust I/O"
    - Removed: "Security & Data Integrity" section (web-specific, not applicable)
    - Removed: "Development Workflow" section (merged into Governance)
  Added sections:
    - Principle V: Workspace Dependency Hygiene
    - Section: Error Handling & Safety
    - Section: Development Workflow
    - Governance (rewritten for Rust workspace)
  Removed sections:
    - Security & Data Integrity (web/RLS-specific)
  Templates requiring updates:
    - .specify/templates/plan-template.md: ✅ No changes needed
      (Constitution Check is generic placeholder)
    - .specify/templates/spec-template.md: ✅ No changes needed
      (success criteria and requirements are generic)
    - .specify/templates/tasks-template.md: ✅ No changes needed
      (phase structure accommodates testing/polish)
  Follow-up TODOs: None
-->

# League Toolkit Constitution

## Core Principles

### I. Crate-First Architecture

- Every format parser MUST live in its own `ltk_*` crate under `crates/`.
- Crates MUST be independently compilable and usable without the umbrella
  `league-toolkit` crate.
- The dependency graph MUST flow upward: foundation crates (`ltk_hash`,
  `ltk_primitives`) have zero internal deps; format crates depend on
  foundation + I/O; higher-level crates depend on format crates.
- Circular dependencies between crates are prohibited.
- The umbrella `league-toolkit` crate MUST re-export sub-crates via feature
  flags only — it MUST NOT contain business logic.

### II. Round-Trip Correctness

- Every format type that supports reading MUST also support writing unless
  the format is documented as read-only.
- Round-trip tests (parse -> write -> parse -> assert equal) are the primary
  verification pattern and MUST exist for all format crates that support
  both reading and writing.
- Floating-point comparisons in tests MUST use the `approx` crate, never
  direct equality.
- Snapshot tests MUST use `insta` with `.ron` format. Snapshot changes MUST
  be reviewed via `cargo insta review` before committing.
- Tests MUST be runnable in isolation — no shared mutable state between
  test cases.

### III. Strict CI Quality Gate

- All code MUST pass `cargo fmt -- --check` with zero differences.
- All code MUST pass `cargo clippy --all-targets -- -D warnings` with zero
  warnings (CI denies all warnings).
- All code MUST pass `cargo test --verbose` with zero failures.
- These three checks MUST pass before any merge to `main`.
- No `#[allow(clippy::*)]` annotations without a documented justification
  in an adjacent comment.

### IV. Idiomatic Rust I/O

- Reading MUST be implemented via `from_reader(&mut impl Read)` (or
  `Read + Seek` when random access is required, e.g., WAD mounting).
- Writing MUST be implemented via `to_writer(&mut impl Write)`.
- Complex types MUST use the builder pattern (e.g., `BinTree::builder()`,
  `RigResource::builder()`, `WadBuilder`).
- All vector/matrix types MUST use `glam` (Vec2, Vec3, Vec4, Mat4, Quat).

### V. Workspace Dependency Hygiene

- Shared dependency versions MUST be declared in the root `Cargo.toml`
  under `[workspace.dependencies]`.
- Individual crates MUST reference workspace dependencies with
  `workspace = true` — pinning a version locally is prohibited unless
  the crate genuinely needs a different version (document why).
- New external dependencies MUST be added at the workspace level first.
- Dependency additions MUST be justified — avoid pulling in large
  dependency trees for trivial functionality.

### VI. Code Style & Formatting

- Separate distinct code contexts (imports, struct definitions, impl blocks,
  helper functions, tests) with a single blank line.
- Within a function, use blank lines to separate logical steps (setup,
  processing, result). Do not insert blank lines between tightly coupled
  sequential statements.
- Group `use` statements by origin: std first, then external crates, then
  internal crate modules, each group separated by a blank line.
- Comments MUST only be added for code with complex or non-obvious behavior.
  Do not document self-explanatory code, simple accessors, or trivial logic.
- Section banners (e.g., `// ── Reading ──`) are allowed to visually separate
  major logical regions within a file. Keep them consistent in style.
- Prefer short, focused functions over long ones. If a function exceeds ~50
  lines, consider whether it can be decomposed.
- `rustfmt` is authoritative for all brace/indent/whitespace decisions —
  do not fight the formatter.

## Error Handling & Safety

- Each crate MUST define its own error type via `thiserror` and a
  `Result<T>` type alias in `error.rs`.
- `ltk_meta` additionally uses `miette` for diagnostic errors; other
  crates MUST NOT add `miette` without justification.
- Unwrap/expect calls are prohibited in library code (non-test).
  Use `?` propagation or explicit error variants.
- `unsafe` blocks MUST include a `// SAFETY:` comment explaining the
  invariant that makes the usage sound.

## Development Workflow

- All changes MUST go through pull requests — direct pushes to `main`
  are prohibited.
- PRs MUST pass the CI quality gate (fmt, clippy, test) before merge.
- Commits MUST use conventional commit format (e.g., `feat(ltk_wad):`,
  `fix(ltk_meta):`, `chore:`, `refactor:`).
- Features MUST be developed on feature branches.
- Hashing algorithms are fixed by the game format: WAD paths use
  XXHash64 (64-bit) of lowercased paths; Bin names use FNV-1a (32-bit)
  via `ltk_hash`. These MUST NOT be changed without format evidence.

## Governance

- This constitution supersedes conflicting practices found elsewhere in
  the codebase. When a conflict is discovered, the constitution takes
  precedence and the conflicting artifact MUST be updated.
- Amendments require: (1) a documented rationale, (2) review of
  downstream impact on templates and specs, and (3) a version bump
  following SemVer.
- Version policy: MAJOR for principle removals or redefinitions, MINOR
  for new principles or material expansions, PATCH for wording
  clarifications.
- Compliance MUST be verified during code review — reviewers SHOULD
  reference specific principles when requesting changes.
- Runtime development guidance lives in `CLAUDE.md` at the repository
  root.

**Version**: 2.1.0 | **Ratified**: 2026-03-07 | **Last Amended**: 2026-03-26
