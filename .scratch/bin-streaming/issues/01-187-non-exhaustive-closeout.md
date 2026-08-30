---
issue: 206
title: "Mark ltk_meta's public error enums #[non_exhaustive] in the 0.8.0 window"
labels: crate:ltk_meta, breaking-change, area:api
---

Before #187 merges, use its 0.8.0 breaking window to mark `ltk_meta`'s public error enums non-exhaustive, so the streaming work (#192, `docs/design/bin-streaming.md` section 13) can add variants in minor releases.

## Proposal

```rust
#[derive(Debug, thiserror::Error, Diagnostic)]
#[non_exhaustive]
pub enum Error { /* unchanged */ }

#[non_exhaustive]
pub enum PropertyPathErrorKind { /* unchanged */ }

#[non_exhaustive]
pub enum ResolveErrorKind { /* unchanged */ }

#[non_exhaustive]
pub enum PatchError { /* unchanged */ }
```

Crate-internal exhaustive matches still compile; downstream matches need a wildcard arm. That is the entire cost, and 0.8.0 is the free moment to pay it.

This issue also acts as the gate for the streaming tickets: none of them start until #187 is merged.

- [x] `Error` carries `#[non_exhaustive]`; crate-internal exhaustive matches still compile
- [x] Decision recorded for `PropertyPathErrorKind` / `ResolveErrorKind` / `PatchError`: all four gained the attribute (design doc section 15.5)
- [x] #187 merged (a32182c); the 0.8.0 release train includes the change (released in #212)
