---
issue: 192
title: "Lazy Bin reading"
labels: area:reading, area:api
---

Implement an API for reading .bin files lazily from a reader.

- Header data reading
- Iterate over all objects in the file by seeking through them. Being able to harvest object path hashes is useful for grepping
- Lazy resolution API - We need to be able to hold a handle for the file to request objects on demand.

The lazy resolution API is crucial for consumers that don't need to parse the whole file to achieve their goal. It's a key blocker for implementing an optimized bin grepping API. Being able to read the object paths and classes without parsing the whole file makes it possible to index efficiently.

## Documents

| What | Where |
| --- | --- |
| Why this exists, who asks for it, requirements, failure modes | `docs/prd/002-streaming-bin-reading.md` |
| API surface, wire format, layout core, testing | `docs/design/bin-streaming.md` |
| Object bytes are buffered and viewed, not streamed through cursors | `docs/adr/0007-object-bytes-are-buffered-and-viewed.md` |
| One layout core under two renderers | `docs/adr/0008-one-layout-core-under-two-renderers.md` |
| A size the counts disagree with is an error | `docs/adr/0009-a-size-the-counts-disagree-with-is-an-error.md` |
| The metadata parameter lives on the handle | `docs/adr/0010-the-metadata-parameter-lives-on-the-handle.md` |
| The object cache is a provider handing out `Arc` | `docs/adr/0011-the-object-cache-is-a-provider-handing-out-arc.md` |
| Tree traits under the walk (the views implement them; #225) | `docs/adr/0014-tree-traits-under-the-walk.md` |

The tickets below render the spec's API-surface sections. Every rule the spec settles is an `Sn`
row in its [section 13](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s13), each naming the ADR that argues it where one does.

## Children

- [x] #206 — Mark ltk_meta's public error enums #[non_exhaustive] in the 0.8.0 window (the gate: the streaming work grows `Error` variants in minors)
- [x] #207 — Bin streaming foundation: mount, TOC, harvest, and the wire core
- [x] #208 — Bin streaming: zero-copy object views
- [x] #209 — Bin streaming: owned decode, the single decode path, and the lookup cache
- [x] #214 — Bin streaming: batch object lookup
- [ ] #210 — Bin streaming: PTCH stream
- [ ] #211 — Bin streaming: delta write-back (the editor's save path)
- [ ] #217 — Bin streaming: resolve a PropertyPath over the views (deliberately unscheduled until a consumer asks — [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s11))

`PROP` reading is complete: mount, header, TOC harvest, per-object seek, zero-copy views, one owned decode path shared with `Bin::from_reader`, the opt-in object cache, and batch lookup. What remains is `PTCH` (#210) and the write side (#211); #217 is the named follow-on [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s11) defers.
