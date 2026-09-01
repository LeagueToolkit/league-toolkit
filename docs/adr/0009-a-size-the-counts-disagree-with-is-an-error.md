# ADR-0009: A declared size the counts disagree with is an error, not a logged discrepancy

- **Status:** Accepted
- **Date:** 2026-08-30
- **Crates:** `ltk_meta`
- **Related:** PRD-002 ([section 6](../prd/002-streaming-bin-reading.md#s6), first row), ADR-0002,
  #192,
  #207, `docs/design/bin-streaming.md` [section 7](../design/bin-streaming.md#s7)

## Context and problem statement

Every complex value in a bin carries a declared byte size ahead of its body, and every complex
value also carries the counts that drive its parse. The two can disagree, and the format does not
say which wins.

**The client trusts counts when parsing and reads sizes only to skip. It never checks that they
agree.** A streaming reader that mirrors the client therefore has a choice the client never had to
make, because the client only ever needs one of the two numbers at a time. A stream needs both:
the size is what a skip seeks by and what every table-of-contents row and `byte_range` is built
from, while the counts are what the parse walks.

The first draft recorded mismatches in a tolerant side-channel - `discrepancies()`,
`discrepancy_count()`, `SizeDiscrepancy` - on the "mirror the client" rationale, and continued.

The corpus settles the cost side: across a full install no shipped file exhibits the mismatch, so
tolerance is bought for files that do not exist and paid for in every file that does.

## Decision drivers

- Diverge from the client only where the divergence is defensible, and say so (this is that place).
- Never let a caller build on offsets the parse has already proven wrong.
- Keep one error for one condition across the eager and streaming paths.

## Considered options

1. **Tolerant log** - record the discrepancy, keep going, let the consumer inspect it.
2. **Raise** - a size that disagrees with the count-driven walk is `Error::InvalidSize`.
3. **Trust the size** - let it win over the counts and reposition the walk.

## Decision

**Option 2. The walk raises `Error::InvalidSize`, the same variant the eager readers have always
raised for this condition.**

[Section 7](../design/bin-streaming.md#s7) specifies the two paths and their trust model, and what
remains usable after a failure - the sequential sweep does not, the already-harvested TOC rows do,
because those offsets tiled correctly up to the failure.

## Consequences

- **Positive:** one condition, one error, on both paths - which is what lets `Bin::from_reader` be
  rebuilt over the stream with its behaviour unchanged. A consumer surveying broken or hand-crafted
  files catches it per chunk, so the tooling is built on the error rather than on state built into
  the core.
- **Negative:** the crate rejects a file the client would load. A file whose sizes and counts
  disagree, in a region the client happens only ever to skip, loads in game and errors here. That
  is the same divergence ADR-0002 takes for patch bins, deliberately and for the same reason.
- **Negative:** there is no partial-read mode for a damaged file beyond what the TOC already
  harvested. A repair tool wanting to salvage past a mismatch has to drive the layout core itself.
- **Revisit when:** a real file appears that the client loads and this rejects. The corpus sweep is
  what would find it.

## Pros and cons of the options

### Option 1: the tolerant log

- Good: mirrors the client's permissiveness; a consumer can decide for itself.
- Bad: a silent-corruption hazard. The TOC and every `byte_range` come from sizes the parse just
  disproved, so "keep going" means handing out offsets known to be wrong, and the side channel
  needs bounding, reporting and testing - complexity bought for files no install contains.

### Option 3: trust the size over the counts

- Good: skips and parses always land on the same next offset by construction.
- Bad: inverts the client's own precedence, so a file the client parses one way would parse another
  way here - a worse divergence than rejecting it.
