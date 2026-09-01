---
issue: 217
title: "Bin streaming: resolve a PropertyPath over the views"
labels: crate:ltk_meta, enhancement, format:bin, area:reading
---

Part of #192 (design: `docs/design/bin-streaming.md` [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s11)). The one piece [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s11) names as a follow-on and never ticketed: walking a `PropertyPath` over the borrowed views, so a consumer that knows where it is going descends only the properties on the path and materializes nothing beside them. The resolver's traversal rules already exist and are corpus-tested (PRD-001); this is those rules run against `ValueView` instead of `PropertyValueEnum`.

## Proposed surface

The path walk is pure once the object's bytes are buffered, so it belongs on the views, and the stream method is the convenience over `view()`:

```rust
impl<'a, M> ObjectView<'a, M> {
    /// The value at `path` inside this object, viewed in place.
    ///
    /// The borrowed mirror of [`BinObject::resolve`], with the same traversal rules: a
    /// name selects a property, `[i]` indexes a list, list2 or option, `{k}` keys a map.
    /// Only the properties on the path are walked; siblings are skipped by size and
    /// nothing is materialized.
    ///
    /// # Errors
    ///
    /// [`Error::Resolve`] with the segment that could not be applied, or the same walk
    /// errors [`ObjectView::property`] raises for malformed bytes.
    pub fn resolve(&self, path: &PropertyPath) -> Result<ValueView<'a, M>, Error>;
}

impl<'a, M> StructView<'a, M> {
    /// See [`ObjectView::resolve`]. Mirrors [`values::Struct::resolve`], including the
    /// null-pointer stop at segment 0.
    pub fn resolve(&self, path: &PropertyPath) -> Result<ValueView<'a, M>, Error>;
}

impl<'a, R: io::Read + io::Seek, M: Default> ObjectStream<'a, R, M> {
    /// Buffers the object and resolves `path` inside it — `view()` plus
    /// [`ObjectView::resolve`].
    pub fn resolve(&mut self, path: &PropertyPath) -> Result<ValueView<'_, M>, Error>;
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// The value at `path` inside object `object_hash`, mirroring [`Bin::resolve`].
    ///
    /// `Ok(None)` when the file holds no such object, matching `object()`; a path that
    /// does not name a value inside one that exists is [`Error::Resolve`].
    pub fn resolve(&mut self, object_hash: impl Into<BinHash>, path: &PropertyPath)
        -> Result<Option<ValueView<'_, M>>, Error>;
}
```

The one new error variant, which `#[non_exhaustive]` (#206) makes a minor-release change:

```rust
pub enum Error {
    // …
    /// A `PropertyPath` did not name a value in the object it was walked through.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
}
```

## Rationale

- **The rules are not rewritten, they are re-rendered.** `walk` in `path/resolve.rs` is the traversal the eager tree uses; the streaming version applies the same segment semantics to `ValueView`, including where a failure is charged (a segment applied to a leaf fails at the segment, not the leaf) and the null-pointer stop. Divergence here is a bug, and the test for it is differential: resolve every corpus path both ways and compare.
- **One error type, not two.** `Error::Resolve` keeps the return `Result<_, Error>` like every other view method, so a caller composing descent with resolution does not juggle two error types. The alternative — `Result<Result<ValueView, ResolveError>, Error>`, separating "the path names nothing" from "the bytes are malformed" — is more precise and worse to hold; `ResolveError` is still recoverable through `Error::Resolve(e) => e.kind()`. Worth confirming before implementing, since the variant is public surface.
- **Read-only.** There is no streaming `resolve_mut` or `patch`: `ValueSlot` mutates an owned tree, and the streaming edit path is `read()` → mutate → `write_patched` (#211). A view is borrowed bytes from the source file and stays that way.
- **`BinStream::resolve` earns its keep over `object()?.resolve()`** only in reading better, so it is a thin forward and can be dropped if it does not.

Unblocked: #208 and #209 shipped the views and the layout core this runs on.

Still unscheduled, and now against a consumer rather than by default. [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s11) defers this until a consumer exists, and the one it was expected to serve was checked against its own code:

- **It already reaches its target in O(1).** The downstream repair path keys objects by path hash and skips every object it holds no finding for, so it never scans for the object. The walk `resolve` would do inside the object it has reached is the work itself, not work saved.
- **It persists no paths.** Its mismatch record is `{ expected, found }`, so there is no stored path grammar a streaming walk would have to stay compatible with, and nothing gets cheaper by front-loading one.
- **The saving needs a caller that does not want the whole tree** — which is the read half of the delta flow, so this couples to #211, parked for reasons of its own.

So it stays a follow-on: open, unscheduled, and thin enough that building it early would fit it to a guess. #192 names the bin grepping API, and the bin editor's link-chasing is a second candidate. Pick it up when one of them walks a path into an object it does not otherwise want, which is what turns the skipped siblings into a saving.

- [ ] `ObjectView::resolve` and `StructView::resolve` walk only the properties on the path; siblings are skipped, nothing is materialized (attested by counting decoded leaves)
- [ ] Differential test: for a sampled set of paths per corpus object, streaming resolve and `BinObject::resolve` agree on the value, and on `ResolveError`'s segment and kind when they fail
- [ ] Every `ResolveErrorKind` is reachable from the streaming walk, including `NullPointer` and `IndexOutOfRange`
- [ ] `BinStream::resolve` returns `Ok(None)` for an absent object and `Err(Error::Resolve(..))` for an absent property, and the distinction is documented
- [ ] `cargo fmt`, `clippy --all-targets`, `doc --no-deps` clean
