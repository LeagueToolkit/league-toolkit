# PRD-002: Streaming bin reading

- **Status:** Approved
- **Created:** 2026-08-31
- **Crates:** `ltk_meta`
- **Tracking:** [#192](https://github.com/LeagueToolkit/league-toolkit/issues/192) (umbrella);
  tickets in `.scratch/bin-streaming/issues/`
- **Spec:** `docs/design/bin-streaming.md`
- **Decisions:** ADR-0007 to ADR-0011

## <a id="s1"></a>1. Problem

**Reading a `.bin` costs the whole file, whatever the caller wanted.** `Bin::from_reader` parses
every property of every object into an owned tree. There is no way to read a header without the
body, no way to reach one object without paying for the rest, and no way to answer a question about
a file's contents that is cheaper than parsing all of it.

The cost is concrete. `PropertyValueEnum` is 96 bytes per node at align 16, so a wire `f32` costs
96 bytes once materialized. A grep index over 42,306 files wants nothing but each object's path
hash and class hash - two `u32`s that sit in the object table's first 8 bytes - and pays a full
parse of every object to get them. A manager resolving one scene object out of a 454,073-object
install pays the same.

The format does not force this. The client's own loader, `MetaFile_readEntry`, is a one-pass
streaming reader: it walks the object table front to back, deserializes each sized entry as it
arrives, and uses the size fields only to seek past what it will not parse. It never builds a
whole-file tree. Streaming is the format's canonical reading model and the eager tree is the
derived convenience, which is the opposite of how the crate is arranged today.

## <a id="s2"></a>2. Objective

A consumer pays for what it reads. Mounting a bin costs its header; harvesting costs one hop per
object; reaching one object costs that object; and a consumer that wants everything still gets
`Bin::from_reader`, now built out of the same parser so the two can never disagree.

## <a id="s3"></a>3. Consumers and stories

- As **bin-grep**, I want every object's path and class hash across 42,306 files without parsing
  their bodies, so that building an index is bounded by I/O rather than by the value model.
- As **`ltk-manager`**, I want to open one object out of a mounted file by path hash, and to keep a
  detached table of contents, so that per-document workers resolve objects without re-reading files.
- As **`ltk-manager`**, I want a batch of objects out of one file scheduled as one pass, so that
  fifty lookups are not fifty seeks in request order.
- As **the bin editor**, I want to expand a file, lazily read one object, edit it, and save, so that
  opening a large bin does not materialize it.
- As **a consumer holding a header**, I want to descend into one value to any depth without
  materializing its siblings, so that reading `Elements[3].Position` costs three lookups.
- As **any caller of `Bin::from_reader`**, I want the eager path to be the streaming path drained,
  so that a fix to one is a fix to both.
- As **`ltk-manager`'s problems pass**, I want to sweep a bin one materialised object at a time,
  and to know the largest object's size before decoding any, so that a streamed bin's memory
  budget is its bytes plus one object's expansion rather than the whole file's - and I want a
  `PTCH` swept the same way for the objects it carries, without reading its records.

## <a id="s4"></a>4. Requirements

### Functional

- **FR-1:** Mounting SHALL read the header, dependencies and class-hash table and stop, without
  touching object bodies.
- **FR-2:** The crate SHALL sweep the object table front to back, yielding one object at a time and
  skipping by the declared size whatever the caller does not descend into.
- **FR-3:** The crate SHALL expose every object's `(path_hash, class_hash, offset, size)` as plain
  data that a consumer can detach from the handle and persist.
- **FR-4:** The crate SHALL open one object by path hash, building whatever index that needs
  transparently on first use.
- **FR-5:** The crate SHALL accept a batch of path hashes up front and visit them in file order, so
  every seek is forward, and SHALL report which of them the file does not contain.
- **FR-6:** Inside an object the crate SHALL support iteration, random access by name hash, and
  descent to any depth, decoding nothing until touched and allocating nothing until an owned value
  is asked for.
- **FR-7:** The crate SHALL upgrade a stream to the owned representation - one object
  (`BinObject`) or the whole file (`Bin`) - on request.
- **FR-8:** `Bin::from_reader` and `BinOverride::from_reader` SHALL be implemented as mount plus
  drain, so the stream is the only parser in the crate.
- **FR-9:** The crate SHALL provide the same treatment for `PTCH` files, including the patch
  records and the outer header's delete list.
- **FR-10:** The crate SHALL offer opt-in per-handle caching of parsed objects, returning values
  that stay valid after eviction.
- **FR-11:** The crate SHALL detect legacy property-kind numbering mid-sweep, recover, and report
  which numbering a handle settled on.
- **FR-12:** A saved edit SHALL be a rewritten `.bin` in which untouched objects are copied through
  byte-exactly and only edited objects are re-encoded.
- **FR-13:** The table of contents SHALL answer the largest declared object size in a file before
  any object body is decoded.
- **FR-14:** A `PTCH` stream SHALL yield its embedded objects through the same cursors as a `PROP`
  stream, without reading its patch records; the records SHALL be reachable only through a cursor
  of their own.

### Non-functional

- **Constant memory at the file level.** Sweeping a bin SHALL NOT hold more than one object's bytes
  at a time, whatever the file's size.
- **No second parser.** Any behaviour the eager reader has - errors included - SHALL be produced by
  the same code the stream uses, not by a parallel implementation that can drift.
- **`Send` handles.** A handle with a cache installed SHALL stay `Send`, for per-document workers.
- **No cost to callers who do not stream.** The existing eager API SHALL keep its signatures and its
  observable behaviour.

## <a id="s5"></a>5. Constraints from the game

Facts the format imposes, read off the client's loader
([section 3](../design/bin-streaming.md#s3) has the detail):

- **The class table is free; path hashes are not.** The object table is `u32 count`, then
  `count x u32 class_hash`, then the bodies. After the sequential header read a handle already holds
  every class hash, but each path hash sits 8 bytes into its object's body - so harvesting is one
  seek-hop per object, and that same hop is what learns each object's `(offset, size)`.
- **Every complex value carries its byte size.** Objects, structs, embeds, containers and maps store
  a size ahead of their body; primitives are fixed width and strings length-prefixed. Skipping any
  unparsed value is therefore a seek, mirroring `MetaValue_skipByType`.
- **The client never verifies sizes on the parse path.** It trusts counts when parsing and reads
  sizes only to skip. Where the toolkit diverges from that is ADR-0009.
- **Legacy property-kind numbering is detectable only by parsing.** Nothing in the header declares
  it. A streaming reader discovers it mid-sweep, which is what forces the latch (FR-11).
- **A `PTCH` is only ever a patch.** It is never loaded as a base file, so streaming it is a
  separate entry point rather than a mode of the `PROP` one.

## <a id="s6"></a>6. Failure modes

| Failure | Cost | What the design owes it |
| --- | --- | --- |
| **A declared size disagrees with the count-driven walk.** A corrupt, truncated or hand-crafted file. | The skip path and the parse path no longer describe the same bytes. Every TOC row and every `byte_range` is built from sizes the parse just proved wrong, so continuing silently corrupts whatever is built on them. | Raise `Error::InvalidSize` on the walk, per ADR-0009. After it, the sequential sweep is untrustworthy; TOC rows harvested before the failure stay valid, because those offsets tiled correctly up to it. |
| **Legacy numbering discovered mid-sweep.** Old files, from before the kind renumbering. | Objects yielded before the discovery were parsed under the wrong mapping - but only ambiguous ones parse cleanly both ways, so the exposure is a prefix of the sweep. | Re-read the current object under the legacy mapping, latch for the handle's life, and report it. `into_bin` restarts from the top so the eager path keeps its exact behaviour. No shipped file in a 392-archive install latches; the latch is a guard, not a path. |
| **A pathological file whose single object is enormous.** | Buffering that object costs its size. | Accepted (ADR-0007). The eager reader materializes a multiple of the same bytes, so this is the smaller footprint everywhere it matters. |

## <a id="s7"></a>7. Out of scope

- **Streaming `resolve(&PropertyPath)`.** The traversal and type rules exist and are corpus-tested
  (PRD-001), and `ValueView` makes the descent thin - which is exactly why it waits for a consumer
  rather than shipping speculatively. Named follow-on, spec
  [section 11](../design/bin-streaming.md#s11).
- **Writing, in v1.** The stream is read-only. The delta-rewrite *contract* is specified (spec
  [section 10](../design/bin-streaming.md#s10)) because the editor's flow depends on its shape; the
  implementation is a later stage.
- **PTCH authoring.** A delta is upstream of either output form, and nothing here forecloses
  rendering one as patch records later.
- **Parallel access within one file.** One cursor at a time per handle, `&mut self` throughout. The
  fan-out workloads parallelize per file.
- **Caching by default.** The uncached paths parse on every call; caching is the opt-in provider of
  FR-10, and measurement says that split is the right one. A hit is 286x cheaper than a miss, but
  the only access pattern a shipped install attests - an editor chasing links - revisits little
  enough to buy 1.2x, while a re-requested working set buys 8.3x. The payoff is the consumer's hit
  rate rather than the crate's, which is what makes `NoCache` the right default and caching the
  wrong thing to impose: `bin-streaming.md` [appendix B](../design/bin-streaming.md#appendix-b) has
  the numbers and the method.

## <a id="s8"></a>8. Acceptance

- [ ] **AC-1:** Mounting a bin reads no object body, and `class_hashes()` is populated.
- [ ] **AC-2:** A full sweep of any file in an install holds at most one object's bytes at a time.
- [ ] **AC-3:** For every `PROP` and `PTCH` chunk in an install: `entries()` harvests the same
      `(path, class)` set the eager parse holds, `into_bin()` equals `Bin::from_reader`, and a
      sampled `object(hash)` equals the eager lookup.
- [ ] **AC-4:** Every declared size in an install equals `PropertyExt::size` over the parsed values,
      attesting that shipped files are size-clean and not merely parse-clean.
- [ ] **AC-5:** `Bin::from_reader` is mount plus drain, with no second parse path in the crate.
- [ ] **AC-6:** A batch request visits the file in offset order and reports its misses.
- [ ] **AC-7:** A file in legacy numbering reads identically through the stream and the eager path.
- [ ] **AC-8:** For every `PTCH` chunk in an install, the object cursors yield the same objects
      the eager `BinOverride::objects` holds and read no byte of the record list (FR-14).
