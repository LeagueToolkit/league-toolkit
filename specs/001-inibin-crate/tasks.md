# Tasks: Inibin Int64 Support + Name Resolution (ltk_inibin_names)

**Input**: Design documents from `/specs/001-inibin-crate/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/
**Context**: `ltk_inibin` already exists with 13 value set types. These tasks add Int64 (flag 13) and a new `ltk_inibin_names` crate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add workspace dependencies and create ltk_inibin_names crate skeleton

- [X] T001 Add `phf` and `phf_codegen` to workspace dependencies in `Cargo.toml` (root)
- [X] T002 Create `crates/ltk_inibin_names/Cargo.toml` with `phf` dependency and `phf_codegen` build-dependency
- [X] T003 Create empty `crates/ltk_inibin_names/src/lib.rs` with module-level doc comment
- [X] T004 Create empty `crates/ltk_inibin_names/build.rs` placeholder

---

## Phase 2: Foundational (Int64 Type Support)

**Purpose**: Add Int64 flag, value variant, and read/write logic to existing ltk_inibin crate

**⚠️ CRITICAL**: Must complete before user story validation

- [X] T005 Add `INT64_LIST = 1 << 13` flag to `InibinFlags` and update `NON_STRING_FLAGS` array in `crates/ltk_inibin/src/flags.rs`
- [X] T006 [P] Add `Int64(i64)` variant to `InibinValue` enum and update `flags()` method in `crates/ltk_inibin/src/value.rs`
- [X] T007 Add Int64List read logic (`read_i64::<LittleEndian>`) to `InibinSet::read_non_string()` in `crates/ltk_inibin/src/set.rs`
- [X] T008 Add Int64List write logic (`write_i64::<LittleEndian>`) to `InibinSet::write_non_string()` in `crates/ltk_inibin/src/set.rs`
- [X] T009 Update `InibinFile::read_v2()` to handle flag bit 13 (Int64List) in the read loop in `crates/ltk_inibin/src/file.rs`
- [X] T010 Update `InibinFile::to_writer()` to include Int64List sets in the write loop in `crates/ltk_inibin/src/file.rs`

**Checkpoint**: Int64 values can be read and written. Existing tests still pass.

---

## Phase 3: User Story 1+2 — Read & Access Int64 Values (Priority: P1)

**Goal**: Parse inibin files containing Int64 sets and access values by key

**Independent Test**: Construct an InibinFile with Int64 values, write to bytes, read back, verify values match

- [X] T011 [P] [US1] Add Int64 unit test to `InibinSet` read tests (test_read_int64_list) in `crates/ltk_inibin/src/set.rs`
- [X] T012 [P] [US2] Add Int64 entries to the `round_trip_all_set_types` test in `crates/ltk_inibin/tests/round_trip.rs`

**Checkpoint**: Int64 read + key access verified by tests

---

## Phase 4: User Story 3+4 — Write & Modify Int64 Values (Priority: P2)

**Goal**: Round-trip Int64 values and support insert/remove operations

**Independent Test**: Insert Int64 values, write, read back, verify round-trip integrity

- [X] T013 [P] [US3] Add dedicated Int64 round-trip test (round_trip_int64) in `crates/ltk_inibin/tests/round_trip.rs`
- [X] T014 [P] [US4] Add Int64 insert/remove test to verify cross-bucket migration in `crates/ltk_inibin/tests/round_trip.rs`

**Checkpoint**: Int64 fully integrated — read, write, modify, round-trip all verified

---

## Phase 5: User Story 5 — Hash Key Name Resolution (Priority: P3)

**Goal**: Provide compile-time hash→(section, name) lookups via `ltk_inibin_names`

**Independent Test**: Query known hashes from the fixlist and verify correct (section, name) pairs returned; query unknown hash and verify None

- [X] T015 [US5] Extract fixlist data from lolpytools `inibin_fix.py` into `crates/ltk_inibin_names/data/fixlist.rs` as a Rust array of `(u32, &str, &str)` tuples
- [X] T016 [US5] Implement `build.rs` in `crates/ltk_inibin_names/build.rs` using `phf_codegen` to generate a `phf::Map<u32, (&str, &str)>` from fixlist data
- [X] T017 [US5] Implement `lookup(hash: u32) -> Option<(&'static str, &'static str)>` in `crates/ltk_inibin_names/src/lib.rs` using the generated phf map
- [X] T018 [US5] Add tests for `lookup()` — verify known hashes return correct pairs and unknown hashes return None in `crates/ltk_inibin_names/src/lib.rs`

**Checkpoint**: Name resolution works for all fixlist entries with zero runtime overhead

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Umbrella integration, CI validation, final cleanup

- [X] T019 [P] Add `inibin-names` feature flag and `ltk_inibin_names` dependency to `crates/league-toolkit/Cargo.toml`
- [X] T020 [P] Add `#[cfg(feature = "inibin-names")] pub use ltk_inibin_names as inibin_names;` re-export in `crates/league-toolkit/src/lib.rs`
- [X] T021 Run `cargo fmt -- --check` across workspace
- [X] T022 Run `cargo clippy --all-targets -- -D warnings` across workspace
- [X] T023 Run `cargo test --verbose` across workspace — all tests must pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T001 for workspace deps)
- **US1+2 (Phase 3)**: Depends on Phase 2 (Int64 type support in place)
- **US3+4 (Phase 4)**: Depends on Phase 2 (can run parallel with Phase 3)
- **US5 (Phase 5)**: Depends on Phase 1 only (T002-T004 for crate skeleton) — independent of Phases 2-4
- **Polish (Phase 6)**: Depends on all prior phases

### User Story Dependencies

- **US1+2 (P1)**: Depends on Foundational — Int64 flag/value/read must exist
- **US3+4 (P2)**: Depends on Foundational — Int64 write must exist. Can run parallel with US1+2
- **US5 (P3)**: Fully independent of US1-4. Only needs crate skeleton from Phase 1

### Parallel Opportunities

- T002, T003, T004 can run in parallel (different files in new crate)
- T005, T006 can run in parallel (different files: flags.rs, value.rs)
- T007, T008 touch same file (set.rs) — must be sequential
- T011, T012 can run in parallel (different test files)
- T013, T014 can run in parallel (same file but independent tests)
- T015 is independent — can start as soon as Phase 1 crate skeleton exists
- T019, T020 can run in parallel (different files)
- Phase 5 (US5) can run entirely in parallel with Phases 3+4

---

## Parallel Example: Phase 5 (US5)

```text
# These can run in parallel with Int64 work (Phases 3+4):
Task T015: "Extract fixlist data into crates/ltk_inibin_names/data/fixlist.rs"
Task T016: "Implement build.rs with phf_codegen"  (depends on T015)
Task T017: "Implement lookup() in lib.rs"          (depends on T016)
Task T018: "Add lookup tests"                      (depends on T017)
```

---

## Implementation Strategy

### MVP First (Int64 Support Only)

1. Complete Phase 1: Setup (workspace deps)
2. Complete Phase 2: Foundational (Int64 type support)
3. Complete Phase 3: US1+2 (Int64 read + access tests)
4. Complete Phase 4: US3+4 (Int64 write + modify tests)
5. **STOP and VALIDATE**: All existing + new tests pass

### Full Delivery

1. MVP (above) + Phase 5: US5 (ltk_inibin_names)
2. Phase 6: Polish (umbrella integration, CI gate)

---

## Notes

- Existing ltk_inibin code (13 types) is already complete and tested
- Int64 follows the exact same pattern as Int32 but with 8-byte values
- The fixlist extraction from Python to Rust is the largest single task (T015) — thousands of entries
- phf_codegen runs at compile time in build.rs, so the generated map is baked into the binary
