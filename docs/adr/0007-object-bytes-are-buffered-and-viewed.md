# ADR-0007: An object's bytes are buffered once and viewed, not streamed through cursors

- **Status:** Accepted
- **Date:** 2026-08-30
- **Crates:** `ltk_meta`
- **Related:** PRD-002 (FR-6), #192, #208,
  `docs/design/bin-streaming.md`
  [section 4.2](../design/bin-streaming.md#s4.2) and [section 4.3](../design/bin-streaming.md#s4.3)

## Context and problem statement

A streaming reader promises constant memory. The first draft took that promise all the way down:
properties were streamed through forward-only lending cursors over the reader, so nothing was ever
held except the one property being looked at.

Measuring the owned value model is what reopened it. `PropertyValueEnum` is 96 bytes per node at
align 16 - a wire `f32` costs 96 bytes materialized - which is the cost the cursors existed to
avoid. But they paid a high price for it: no `std` iterators, one property at a time, no
backtracking, no holding two properties to compare them, and lazy descent had to be deferred
entirely behind a reserved `value_range()` door.

The fact that changes the calculation is that **an object's size is known before descending into
it, and objects are KB-scale**. Buffering one object bounds memory just as tightly as the cursors
did, at a granularity the format hands over for free.

## Decision drivers

- Keep the constant-memory guarantee where it actually matters - the file-level sweep, which is
  what makes a 42,306-file harvest bounded.
- Do not make read-only consumers pay the 96-byte node cost at all.
- Prefer a surface a caller can use with `std` idioms over one that forces a bespoke loop.

## Considered options

1. **Forward-only lending cursors over the reader** - the first draft; nothing buffered.
2. **Buffer the object's declared byte range, view it zero-copy** - `std` iterators and random
   access in memory.
3. **Ship both** - cursors for the strict case, views for the convenient one.
4. **Views that delegate to the existing `io::Read` readers through `io::Cursor`.**

## Decision

**Option 2. Descending buffers the object's declared range into a handle-owned reused buffer, and
everything inside the object happens in memory.**

The file level is untouched: the object-table sweep still streams and skips by size, which is where
the constant-memory guarantee lives. [Section 4.2](../design/bin-streaming.md#s4.2) and
[section 4.3](../design/bin-streaming.md#s4.3) specify the surface this produces, and
[section 8](../design/bin-streaming.md#s8) the consequence for the numbering latch - the retry
becomes a re-walk of bytes already in memory rather than I/O.

## Consequences

- **Positive:** a read-only consumer materializes nothing, so the 96-byte node cost is simply not
  paid. Descent to any depth becomes the natural surface instead of a reserved door, which turns
  the streaming resolver from a speculative feature into a thin follow-on. Every cursor restriction
  dissolves at once.
- **Negative:** a pathological file whose single object is enormous buffers that whole object. The
  eager reader materializes a multiple of the same bytes, so this is still the smaller footprint
  everywhere it matters, but it is no longer a hard bound independent of the file's shape.
- **Negative:** the walk is not lazy even though the viewing is. The latch has to be settled before
  any view is handed out and a view cannot flip it from behind a shared reference, so buffering
  walks the object once as it lands.
- **Revisit when:** a corpus appears with objects large enough that one does not fit comfortably in
  memory. Nothing in a shipped install comes close.

## Pros and cons of the options

### Option 1: forward-only lending cursors

- Good: the tightest possible memory bound, at property granularity.
- Bad: bought with every ergonomic property of the surface - no `std` iterators, no backtracking,
  one property at a time - to save bytes the object's own size field already bounds.

### Option 3: ship both

- Good: neither consumer compromises.
- Bad: two per-object models to keep in agreement forever, and views subsume every use the cursor
  had. The second model is cost with no remaining customer.

### Option 4: views delegating through `io::Cursor`

- Good: reuses the readers that already exist, no second codec family.
- Bad: an owned allocation per leaf - zero-copy in name only, which is the entire point.
