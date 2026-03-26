# Tasks: ltk_inibin PR #122 Review Fixes

**Input**: Design documents from `/specs/001-inibin-crate/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/
**Context**: `ltk_inibin` crate is fully implemented. These tasks address review comments from `alanpq` on PR #122.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: No setup needed — all crates and dependencies already exist.

(No tasks — crate structure is already in place)

---

## Phase 2: Foundational (Renames & Signature Changes)

**Purpose**: Rename types and update function signatures that all other changes depend on

**⚠️ CRITICAL**: Must complete before user story work begins

- [X] T001 Rename `ValueKind` to `ValueFlags` in `crates/ltk_inibin/src/value_kind.rs` — update the `bitflags!` struct name, `NON_STRING_KINDS` constant, and all doc comments
- [X] T002 Rename file `crates/ltk_inibin/src/value_kind.rs` to `crates/ltk_inibin/src/value_flags.rs` and update the module declaration in `crates/ltk_inibin/src/lib.rs` from `mod value_kind` to `mod value_flags`
- [X] T003 Update `pub use value_kind::ValueKind` to `pub use value_flags::ValueFlags` in `crates/ltk_inibin/src/lib.rs`
- [X] T004 [P] Update all references from `ValueKind` to `ValueFlags` in `crates/ltk_inibin/src/section.rs`
- [X] T005 [P] Update all references from `ValueKind` to `ValueFlags` in `crates/ltk_inibin/src/file.rs`
- [X] T006 [P] Update all references from `ValueKind` to `ValueFlags` in `crates/ltk_inibin/src/value.rs`
- [X] T007 [P] Update all references from `ValueKind` to `ValueFlags` in `crates/ltk_inibin/tests/round_trip.rs`
- [X] T008 [P] Update all references from `ValueKind` to `ValueFlags` in any example files under `crates/ltk_inibin/examples/`
- [X] T009 Change SDBM hash function signatures from `&str` to `impl AsRef<str>` in `crates/ltk_hash/src/sdbm.rs` — update `hash_lower(input: impl AsRef<str>)` and `hash_lower_with_delimiter(a: impl AsRef<str>, b: impl AsRef<str>, delimiter: char)`, adding `.as_ref()` calls on usage sites within the function bodies
- [X] T010 Update SDBM hash tests in `crates/ltk_hash/src/sdbm.rs` to verify both `&str` and `String` arguments work

**Checkpoint**: `ValueKind` → `ValueFlags` rename complete everywhere. SDBM functions accept `AsRef<str>`. `cargo check` passes.

---

## Phase 3: User Story 1+2 — Unified Value Accessors (Priority: P1) 🎯 MVP

**Goal**: Replace separate packed-float accessor methods with unified `as_*()` methods that handle both packed (U8-based) and non-packed (F32) variants transparently.

**Independent Test**: Call `as_f32()` on both `Value::F32(1.5)` and `Value::U8(15)` — both should return `Some(f32)`. Same pattern for vec types.

### Implementation

- [X] T011 [US1] Replace `u8_as_f32()` with `as_f32()` in `crates/ltk_inibin/src/value.rs` — return `Some(v)` for `F32(v)`, `Some(b as f32 * 0.1)` for `U8(b)`, `None` for other variants
- [X] T012 [P] [US1] Replace `vec2_u8_as_f32()` with `as_vec2()` in `crates/ltk_inibin/src/value.rs` — return `Some(v)` for `Vec2F32(v)`, `Some(Vec2::new(a*0.1, b*0.1))` for `Vec2U8([a,b])`, `None` for other variants
- [X] T013 [P] [US1] Replace `vec3_u8_as_f32()` with `as_vec3()` in `crates/ltk_inibin/src/value.rs` — return `Some(v)` for `Vec3F32(v)`, `Some(Vec3::new(a*0.1, b*0.1, c*0.1))` for `Vec3U8([a,b,c])`, `None` for other variants
- [X] T014 [P] [US1] Replace `vec4_u8_as_f32()` with `as_vec4()` in `crates/ltk_inibin/src/value.rs` — return `Some(v)` for `Vec4F32(v)`, `Some(Vec4::new(a*0.1, b*0.1, c*0.1, d*0.1))` for `Vec4U8([a,b,c,d])`, `None` for other variants
- [X] T015 [US2] Update all call sites of the old accessor names (`u8_as_f32`, `vec2_u8_as_f32`, etc.) across `crates/ltk_inibin/src/` to use the new unified names (`as_f32`, `as_vec2`, etc.)
- [X] T016 [US2] Update any example files under `crates/ltk_inibin/examples/` that reference old accessor names

**Checkpoint**: Unified `as_*()` accessors work for both packed and non-packed variants. No references to old method names remain.

---

## Phase 4: User Story 3+4 — Section Iterator Methods (Priority: P2)

**Goal**: Add `.keys()` and `.values()` iterator methods to `Section` for idiomatic map-like access.

**Independent Test**: Parse an inibin, get a section, call `.keys()` and `.values()` — verify they return the expected hash keys and values respectively.

### Implementation

- [X] T017 [US3] Add `pub fn keys(&self) -> impl Iterator<Item = &u32>` to `Section` in `crates/ltk_inibin/src/section.rs` — delegate to `self.properties.keys()`
- [X] T018 [US3] Add `pub fn values(&self) -> impl Iterator<Item = &Value>` to `Section` in `crates/ltk_inibin/src/section.rs` — delegate to `self.properties.values()`
- [X] T019 [US4] Review and remove duplicate helper at approximately line 353 in `crates/ltk_inibin/src/section.rs` — check if existing `ltk_io_ext` or other utility already provides this functionality. If duplicate found, replace with existing helper; if not, leave as-is with a comment

**Checkpoint**: `Section` exposes `.keys()`, `.values()`, `.iter()`. No duplicate helpers remain.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: CI validation, verify round-trip tests still pass, final cleanup

- [X] T020 Run `cargo fmt -- --check` across workspace
- [X] T021 Run `cargo clippy --all-targets -- -D warnings` across workspace — fix any new warnings from the renames
- [X] T022 Run `cargo test --verbose` across workspace — all existing and new tests must pass
- [X] T023 Verify round-trip test in `crates/ltk_inibin/tests/round_trip.rs` still passes with `ValueFlags` rename
- [X] T024 [P] Update `crates/ltk_inibin/README.md` if it references `ValueKind` or old accessor names

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 2)**: No dependencies — start immediately
  - T001-T003 must be sequential (rename struct, rename file, update re-export)
  - T004-T008 can run in parallel after T001-T003 (updating references in different files)
  - T009-T010 can run in parallel with T001-T008 (different crate: `ltk_hash`)
- **US1+2 (Phase 3)**: Depends on Phase 2 (T006 — `ValueFlags` rename in value.rs must be done first)
  - T011-T014 can run in parallel (different methods in same file, but independent)
  - T015-T016 depend on T011-T014 (need new names to exist before updating call sites)
- **US3+4 (Phase 4)**: Depends on Phase 2 (T004 — `ValueFlags` rename in section.rs must be done first). Can run in parallel with Phase 3.
  - T017-T018 can run in parallel (different methods)
  - T019 is independent
- **Polish (Phase 5)**: Depends on all prior phases

### Parallel Opportunities

- T004, T005, T006, T007, T008 can all run in parallel (different files)
- T009-T010 (`ltk_hash` changes) can run in parallel with all `ltk_inibin` changes
- Phase 3 and Phase 4 can run in parallel (different files: value.rs vs section.rs)
- T012, T013, T014 can run in parallel (independent method implementations)
- T017, T018 can run in parallel (independent methods)

---

## Parallel Example: Phase 2 (Foundational)

```text
# Sequential first (rename chain):
Task T001: "Rename ValueKind to ValueFlags in value_kind.rs"
Task T002: "Rename file to value_flags.rs, update module declaration"
Task T003: "Update pub use in lib.rs"

# Then parallel (reference updates across files):
Task T004: "Update ValueKind → ValueFlags in section.rs"
Task T005: "Update ValueKind → ValueFlags in file.rs"
Task T006: "Update ValueKind → ValueFlags in value.rs"
Task T007: "Update ValueKind → ValueFlags in round_trip.rs"
Task T008: "Update ValueKind → ValueFlags in examples/"

# Parallel with all above (different crate):
Task T009: "AsRef<str> for SDBM hash functions"
Task T010: "Update SDBM tests"
```

---

## Implementation Strategy

### MVP First (Rename + Accessors)

1. Complete Phase 2: Foundational (ValueFlags rename + SDBM AsRef)
2. Complete Phase 3: US1+2 (unified as_*() accessors)
3. **STOP and VALIDATE**: `cargo test` passes, review comments addressed

### Full Delivery

1. MVP (above) + Phase 4: US3+4 (.keys()/.values() + helper dedup)
2. Phase 5: Polish (CI gate, README update)

---

## Notes

- All changes are refactoring/API improvements — no new binary format logic
- The `ValueKind` → `ValueFlags` rename touches many files but is mechanical
- Unified `as_*()` accessors replace existing methods — this is NOT additive, old names are removed
- The duplicate helper check (T019) may result in no change if no duplicate is found
- Round-trip tests are the primary regression gate — must pass after all changes
