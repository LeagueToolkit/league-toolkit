---

description: "Task list template for bug fix implementation"
---

# Tasks: Fix [BUG TITLE]

**Input**: Fix plan from `/specs/[###-fix-short-name]/`
**Prerequisites**: plan.md (required), spec.md (required for reproduction & verification)

**Organization**: Tasks follow investigation → fix → verify flow. Bug fixes are typically small and sequential.

## Format: `[ID] [P?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- Include exact file paths in descriptions

<!--
  ============================================================================
  IMPORTANT: The tasks below are SAMPLE TASKS for illustration purposes only.

  The /speckit.bugfix-tasks command MUST replace these with actual tasks based on:
  - Root cause from plan.md
  - Files to modify from plan.md
  - Verification criteria from spec.md
  - Testing strategy from plan.md

  Bug fix tasks are typically fewer and more focused than feature tasks.

  DO NOT keep these sample tasks in the generated tasks.md file.
  ============================================================================
-->

## Phase 1: Reproduce & Confirm

**Purpose**: Verify the bug exists and is reproducible before making changes

- [ ] T001 Reproduce the bug following steps in spec.md
- [ ] T002 Identify the exact code path triggering the bug
- [ ] T003 [P] Write a failing test that demonstrates the bug in tests/[path]

---

## Phase 2: Fix Implementation

**Purpose**: Apply the minimal fix identified in plan.md

- [ ] T004 [Fix description] in [file path]
- [ ] T005 [Additional fix if needed] in [file path]

**Checkpoint**: Bug should no longer be reproducible via the original steps

---

## Phase 3: Verification & Regression

**Purpose**: Confirm the fix works and nothing else broke

- [ ] T006 Verify the failing test from T003 now passes
- [ ] T007 Run existing test suite to check for regressions
- [ ] T008 [P] Manual verification following spec.md success criteria
- [ ] T009 [P] Test edge cases identified in plan.md

---

## Phase 4: Cleanup (if needed)

**Purpose**: Any follow-up improvements directly related to the fix

- [ ] T010 [P] Update documentation if behavior changed
- [ ] T011 [P] Add additional test coverage for related edge cases

---

## Dependencies & Execution Order

### Phase Dependencies

- **Reproduce (Phase 1)**: No dependencies - start immediately
- **Fix (Phase 2)**: Depends on Phase 1 confirmation
- **Verification (Phase 3)**: Depends on Phase 2 completion
- **Cleanup (Phase 4)**: Depends on Phase 3 passing

### Parallel Opportunities

- T003 (write failing test) can run in parallel with T001-T002 if root cause is known
- Verification tasks marked [P] can run in parallel
- Cleanup tasks marked [P] can run in parallel

---

## Notes

- Bug fixes should be minimal and focused - fix the bug, not surrounding code
- Always write a regression test before or alongside the fix
- Verify non-regression criteria from spec.md before marking complete
- Commit after each logical step for easy bisect/revert
