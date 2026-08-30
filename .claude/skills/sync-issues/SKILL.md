---
name: sync-issues
description: Push ticket files and design-doc changes to their GitHub issues so the issues never drift from the repo. Use this whenever a design doc under docs/design/ or a ticket file under .scratch/*/issues/ has been created or edited, whenever the user asks to sync, update, publish, or create GitHub issues from tickets or a design doc, and at the end of any task that touched those files — even if the user did not mention issues. Also use it when a new ticket file needs a GitHub issue created for it.
---

# Sync tickets to GitHub issues

The repo is the single source of truth for planned work. Ticket files under
`.scratch/<project>/issues/*.md` (and the design docs under `docs/design/` they draw from)
are what gets edited; the GitHub issues are rendered artifacts of them. Nobody hand-edits
an issue body on GitHub — if an issue and its ticket disagree, the ticket wins, and this
skill's job is to make GitHub match. That rule is what makes "which copy is current?" a
question that never needs asking.

## The mapping

Each ticket file carries YAML frontmatter binding it to its issue, and everything below
the frontmatter is the issue body, pushed verbatim:

```yaml
---
issue: 207
title: "Bin streaming foundation: mount, TOC, harvest, and the wire core"
labels: crate:ltk_meta, enhancement, format:bin, area:reading, blocked
---
```

`title` and `labels` apply on creation; on an existing issue only the body is pushed
(titles stay stable — see below). Because the body is verbatim, strip the frontmatter
before pushing (`sed '1,/^---$/d'` deletes line 1 through the closing `---`) and never put
an H1 title heading in the body — the issue title already renders above it.

A ticket without an `issue:` key has no GitHub issue yet — create one (see below) and
write the number back into the ticket's frontmatter immediately. That write-back is part
of creating the issue, not a follow-up: a created-but-unrecorded issue is how duplicates
happen on the next sync.

## Procedure

1. Determine which tickets are affected. If the session edited specific tickets or
   design-doc sections, sync those; if the user asked for a full sync, sync every ticket
   in the directory. Don't skip a ticket because its own file is unchanged when the
   design-doc sections it renders from changed.
2. For each affected ticket, render the issue body (format below) to a file in the
   scratchpad, then push it:
   - Existing issue: `gh issue edit <N> --repo LeagueToolkit/league-toolkit --body-file <file>`
   - New issue: `gh issue create --repo LeagueToolkit/league-toolkit --title "<title>" --label "<labels>" --body-file <file>`,
     then write the returned number into the ticket's frontmatter.
3. Verify each push succeeded (gh prints the issue URL). Report the synced issue numbers.

Titles stay stable across syncs — renaming an issue breaks people's links and
notifications, so change a title only when the user asks.

## Issue body format

Issues are **API proposals**, written the way the design docs write their API-surface
sections (see `docs/design/bin-streaming.md` §4–§5 for the model). Structure, in order:

1. One short paragraph: what this delivers and which umbrella issue / design doc it
   belongs to (e.g. "Part of #192 (design: `docs/design/bin-streaming.md` §5)").
2. `## Proposed surface` — Rust signature blocks lifted from the design doc, not
   paraphrased into prose. Keep the abbreviated doc comments; elide private internals
   with `/* … */`. If the design doc has the signatures, extract them verbatim; prose
   summaries of an API are exactly the drift this skill exists to prevent.
3. Rationale and invariants as short prose or bullets — only what a reader needs to
   evaluate the proposal, not the design doc's full narrative.
4. A `Blocked by #N` line naming real issue numbers (resolve them through the frontmatter
   mapping of the tickets it depends on).
5. The acceptance checklist (`- [ ]` items), last.

Labels follow the existing scheme: `crate:ltk_*` for the crate, `enhancement`/`bug`,
`format:*`, `area:reading`/`area:api`/`area:writing`, `breaking-change` where it applies,
`blocked` while a `Blocked by` line names an open issue.

## Voice

Everything published to GitHub must read as written by the repo owner. No AI attribution
of any kind: no "Generated with Claude Code" footers, no Co-Authored-By lines, no session
links, no mention of agents or assistants — in issue bodies, titles, or comments. Plain
technical register, matching the design docs.

## What not to do

- Don't edit issue bodies with inline `--body` strings; Rust snippets full of quotes and
  backticks mangle in shell quoting. Always go through `--body-file`.
- Don't close, reopen, or comment on issues — this skill only creates bodies and keeps
  them current. State transitions are the maintainer's.
- Don't "improve" ticket content while syncing. Rendering is mechanical; if a ticket
  seems wrong, tell the user instead of silently fixing it on GitHub.
