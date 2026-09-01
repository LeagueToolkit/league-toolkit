# ADR-0008: One layout core sits under both renderers

- **Status:** Accepted
- **Date:** 2026-08-30
- **Crates:** `ltk_meta`
- **Related:** PRD-002 (FR-8, and the no-second-parser requirement), #192, #207, #209,
  `docs/design/bin-streaming.md` [section 9](../design/bin-streaming.md#s9)

## Context and problem statement

Once borrowed views exist there are two ways to decode a value: borrowed out of bytes, and owned
into `PropertyValueEnum`. Two decoders over one wire format drift - not immediately, but on the
first edge case one of them handles and the other does not, and the format has plenty (legacy kind
numbering, sized regions, non-UTF-8 strings, nested containers the format forbids).

The crate already had one decoder, the `ReadProperty` impls, reading `io::Read` directly. Adding a
second for the views would have meant two answers to "how far does this value reach" - the exact
question every skip, every view and every parse depends on.

## Decision drivers

- A behaviour the eager reader has must be produced by the code the stream uses, not by a parallel
  implementation.
- "How far does this value reach" must have exactly one answer in the crate.
- A `ValueView::String` and an owned `values::String` must never disagree about the same bytes.

## Considered options

1. **A separate streaming parser** beside the existing `ReadProperty` impls.
2. **One byte-level layout core, with the owned impls and the views as renderers over it.**
3. **Views delegating to the owned impls**, materializing and discarding.

## Decision

**Option 2. One crate-internal module owns the layout - where a value starts, how far it runs, what
its header declares, and the leaf codecs over `&[u8]` - and both surfaces render over it.**

`Bin::from_reader` becomes mount plus drain, so the stream is the crate's only parser. Section
[section 9](../design/bin-streaming.md#s9) specifies the module, the reader bridge the closed
`ReadProperty` breaking window forced, and the two divergences the renderers deliberately keep.

## Consequences

- **Positive:** the single-decode-path rule holds one level below where it used to, so it now
  covers the views as well as the two entry points. The corpus parity sweep has something to
  assert against, and inline size checks that lived in the `ReadProperty` impls move to one place.
- **Negative:** `ReadProperty`'s signature is public and its breaking window is closed, so the
  impls still take an `io::Read + io::Seek` and need a bridge: for the self-sized kinds they grow a
  buffer until the layout core can cross it, then wind the reader back over the over-read. That is
  a real piece of machinery that exists only because the trait cannot change.
- **Negative:** two behaviour changes fell out where the byte-level codec's answer replaced the
  reader's - a non-UTF-8 string now raises `Error::Utf8Error` rather than `Error::ReaderError`, and
  `Bin::from_reader` buffers internally so it no longer leaves the reader at a defined position.
  Neither had a caller in the workspace, and both are documented.
- **Revisit when:** `ReadProperty` gets a breaking window. The bridge is the thing to delete.

## Pros and cons of the options

### Option 1: a separate streaming parser

- Good: no bridge, no refactor of the existing impls, ships sooner.
- Bad: two answers to every extent question, and the drift is silent - the two parsers agree on
  every shipped file right up until the one they do not, which is a corrupt file nobody is testing.

### Option 3: views delegating to the owned impls

- Good: trivially consistent, one decoder.
- Bad: materializes to answer a borrowed question, which deletes the reason views exist.
