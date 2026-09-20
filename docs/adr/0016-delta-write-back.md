# ADR-0016: Delta write-back

- **Status:** Accepted
- **Date:** 2026-09-14
- **Crates:** `ltk_meta`
- **Related:** PRD-002 (FR-12), ADR-0007, ADR-0015, #211,
  `docs/design/bin-streaming.md` [section 10](../design/bin-streaming.md#s10)

## Context and problem statement

A repair and a save edit a few objects of a bin and write the whole file. The eager path to a
written file is `into_bin` or `Bin::from_reader`, then `Bin::to_writer`: every object of the file is
decoded into `PropertyValueEnum` values, 96 bytes per node, and every object is encoded again.

`BinStream` holds each object's byte range in the TOC. An object nobody edited is bytes the
writer can copy without reading them as values. `Bin::to_writer` writes version 3 whatever the
source version was.

The case against a delta write (#211): the manager re-encodes the whole file and loses nothing it
did not address, a whole-file transcode is a supported save, and a delta write adds a refusal case
for a legacy-numbered base that a transcode does not have.

A streamed read has one object's expansion in memory at a time (ADR-0007). A transcode holds the
whole tree.

## Decision drivers

- The cost of a save scales with the edited objects, not the file.
- A save through the stream holds one edited object's expansion at a time, not the file's tree.
- Nothing the crate cannot interpret is lost from an object nobody edited.
- A save does not change the file's version.
- A file with mixed kind numbering is never written.

## Considered options

1. **Whole-file transcode only** - `into_bin`, edit the tree, `Bin::to_writer`.
2. **Delta rewrite** - a `BinDelta` of replaced, removed and appended objects; `write_patched`
   copies every untouched object's byte range and encodes only the objects the delta names.
3. **In-place byte patching** - a mutable view over an object's bytes, written back into the
   original file.

## Decision

**Option 2. A save through the stream is `BinStream::write_patched` over a `BinDelta`: the header
and class table are rebuilt, every object the delta does not name is copied byte for byte from its
TOC range, and replaced and appended objects are encoded through the eager writer.** The surface,
the invariants and the errors are `docs/design/bin-streaming.md`
[section 10](../design/bin-streaming.md#s10).

A base read under the legacy numbering refuses the write with a dedicated error naming the
transcode. A delta naming an object the base does not hold, or appending an object the output
also holds, is an error before any byte is written.

## Consequences

- **Positive:** a one-object edit reads the file's header and TOC and that object, and writes the
  rest as copies: 1.73 s against 10.44 s for a transcode, summed over the 49,291 `PROP` chunks of a
  16.18 install (`bin-streaming.md` [appendix C](../design/bin-streaming.md#appendix-c)). An
  untouched object keeps every byte, a kind with no widget and a hash no table names included.
  The version passes through. The manager's repair and the editor's save share one writer, and
  the edit itself is ADR-0015's `walk_mut` over the object `read()` returned.
- **Negative:** a refusal case a transcode does not have, for a legacy-numbered base, and a
  fallback the consumer carries for it. The latch settles only as objects are read, and a base whose
  legacy objects are all unread writes without refusing. No shipped file latches (`bin-streaming.md`
  [appendix A](../design/bin-streaming.md#appendix-a)). A second write path beside `Bin::to_writer`,
  with its own tests.
- **Revisit when:** a shipped file latches onto the legacy numbering. The alternative at that point
  is a delta write that verifies the numbering of every object it copies, at the cost of a walk per
  object.

## Pros and cons of the options

### Option 1: whole-file transcode only

- Good: one writer, no refusal case, and a legacy base comes out in the current numbering.
- Bad: a one-property edit decodes and encodes every object, holds the whole tree, and writes
  version 3 over a version 1 or 2 file. Tempting as the existing path, and the one the manager
  calls.

### Option 3: in-place byte patching

- Good: no copy of the untouched bytes at all.
- Bad: only a fixed-width leaf keeps its size. A string edit shifts every size field above it, and
  an added property shifts the whole object; the write degrades to a rewrite for most edits anyway.
  A mutable view is a second value model over bytes, and `bin-streaming.md`
  [section 10.4](../design/bin-streaming.md#s10.4) rules it out.
