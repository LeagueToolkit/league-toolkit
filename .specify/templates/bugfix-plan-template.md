# Fix Plan: [BUG TITLE]

**Branch**: `[###-fix-short-name]` | **Date**: [DATE] | **Bug Report**: [link]
**Input**: Bug report from `/specs/[###-fix-short-name]/spec.md`

**Note**: This template is filled in by the `/speckit.bugfix-plan` command.

## Summary

[One-line: what's broken and the planned fix approach]

## Root Cause Analysis

<!--
  ACTION REQUIRED: Document the investigation process and findings.
  This section is the most critical part of a bug fix plan.
-->

### Investigation Steps

1. [What was checked first and what was found]
2. [What was checked next and what was found]
3. [How the root cause was identified]

### Root Cause

**Location**: [File path(s) and line number(s) where the bug originates]
**Mechanism**: [Technical explanation of why the bug occurs]
**Trigger**: [What conditions cause the bug to manifest]

### Contributing Factors

- [e.g., "Missing null check on user input"]
- [e.g., "Race condition between two async operations"]
- [e.g., "RLS policy doesn't account for this user state"]

## Fix Strategy

<!--
  ACTION REQUIRED: Choose and document the fix approach.
  Prefer the simplest fix that addresses the root cause.
-->

### Approach

[Describe the fix at a high level - what will change and why]

### Files to Modify

| File | Change | Reason |
|------|--------|--------|
| [path/to/file] | [Brief description of change] | [Why this change fixes the bug] |
| [path/to/file] | [Brief description of change] | [Why this change fixes the bug] |

### Approach Alternatives Considered

| Alternative | Why Rejected |
|-------------|-------------|
| [Alternative fix 1] | [Why it's not the best approach] |
| [Alternative fix 2] | [Why it's not the best approach] |

## Risk Assessment

### Fix Scope

- **Minimal**: Change is isolated to [X files/functions]
- **Side effects**: [None expected / List potential side effects]
- **Rollback plan**: [How to revert if the fix causes issues]

### Testing Strategy

- **Unit test**: [What unit test to add/modify to prevent regression]
- **Manual verification**: [Steps to manually verify the fix]
- **Edge cases**: [Edge cases to test around the fix]

## Constitution Check

*GATE: Verify fix aligns with project principles.*

[Check against constitution - especially around testing, code quality, and security]

## Technical Context

**Language/Version**: [from existing project]
**Affected Components**: [e.g., "mod-install service, RLS policies"]
**Storage**: [if applicable]
**Testing**: [e.g., "Vitest unit test + manual verification"]
