# Bug Report: [BUG TITLE]

**Feature Branch**: `[###-fix-short-name]`
**Created**: [DATE]
**Status**: Draft
**Severity**: [Critical / High / Medium / Low]
**Input**: User description: "$ARGUMENTS"

## Problem Statement *(mandatory)*

<!--
  Describe the bug clearly and concisely. Focus on WHAT is broken,
  not WHY (that comes in the plan/investigation phase).
-->

[Clear description of what is broken or behaving incorrectly]

## Reproduction Steps *(mandatory)*

<!--
  ACTION REQUIRED: Replace with concrete reproduction steps.
  Each step should be specific enough that anyone can reproduce.
-->

### Environment

- **Where observed**: [Production / Staging / Local development]
- **Browser/Client**: [if applicable]
- **User role/state**: [if applicable, e.g., "authenticated admin user"]

### Steps to Reproduce

1. [First action]
2. [Second action]
3. [Third action]
4. Observe: [what happens]

### Expected Behavior

[What should happen instead]

### Actual Behavior

[What actually happens, including error messages, screenshots, or logs if available]

### Reproduction Rate

- [ ] Always reproducible
- [ ] Intermittent (approximate frequency: [X out of Y attempts])
- [ ] Only under specific conditions: [describe conditions]

## Impact Assessment *(mandatory)*

<!--
  Quantify the impact. Who is affected and how badly?
-->

### Affected Users

- **Scope**: [All users / Subset: describe who]
- **Workaround available**: [Yes: describe / No]
- **Data loss or corruption risk**: [Yes: describe / No]

### Business Impact

- [e.g., "Users cannot complete mod installation, blocking core functionality"]
- [e.g., "Affects approximately X% of active users"]
- [e.g., "No workaround exists, feature is completely broken"]

## Related Context *(include if known)*

<!--
  Any additional context that might help investigation.
  Remove this section entirely if nothing is known.
-->

- **When it started**: [Date/version if known, or "unknown"]
- **Recent changes**: [Any deployments, migrations, or config changes around that time]
- **Related issues**: [Links to related bugs or features]
- **Error logs**: [Relevant log entries, stack traces, or error codes]

## Success Criteria *(mandatory)*

<!--
  How do we know the bug is fixed? These must be verifiable.
-->

### Fix Verification

- **FV-001**: [Measurable verification, e.g., "User can complete mod installation without error"]
- **FV-002**: [Measurable verification, e.g., "No 500 errors in logs for this endpoint over 24 hours"]
- **FV-003**: [Regression check, e.g., "Existing mod installations continue to work correctly"]

### Non-Regression

- [List related functionality that must continue working after the fix]
- [e.g., "Other mod actions (edit, delete, publish) remain unaffected"]
