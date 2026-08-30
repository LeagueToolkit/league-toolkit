---
issue: 214
title: "Bin streaming: batch object lookup"
labels: crate:ltk_meta, enhancement, format:bin, area:reading, blocked
---

Part of #192 (design: `docs/design/bin-streaming.md` section 4.5). `object(hash)` answers one question per seek; a consumer that wants fifty objects out of one bin pays fifty seeks in whatever order it asked. `objects_batch` takes the whole request up front so the handle can schedule the I/O.

## Proposed surface

```rust
impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Opens the objects with the given path hashes, visiting them in file order.
    ///
    /// Takes the whole request up front so the reads can be scheduled: before the
    /// TOC exists, the requests resolve during its one forward scan of the object
    /// table, which stops as soon as every requested hash is found; with the TOC
    /// built, the requested entries are visited in offset order, so every seek is
    /// forward. Duplicate hashes in the request resolve once.
    pub fn objects_batch(
        &mut self,
        hashes: impl IntoIterator<Item = impl Into<BinHash>>,
    ) -> BatchObjects<'_, R, M>;
}

/// Lending cursor over a requested set of objects, in file order.
#[must_use = "cursors are lazy and read nothing until advanced"]
pub struct BatchObjects<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> BatchObjects<'a, R, M> {
    /// Advances to the next requested object the table contains.
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error>;

    /// The requested hashes the object table does not contain.
    ///
    /// Complete once `next` has returned `Ok(None)`; before that it only holds
    /// what the scan has already ruled out.
    pub fn missing(&self) -> &[BinHash];
}
```

The decisions, from the design doc:

- **The schedule key is the file offset, never the hash.** Hash order has no relationship to where objects sit in the file, so sorting a request by hash would still seek randomly. Offset order is what the internal buffer and the OS readahead reward — and it is also simply file order, which is why the cold and the warm path can promise the same yield order.
- **Yield order is file order, documented.** A caller that needs request order collects and reorders — it has the hashes. Promising request order would force the handle back into random seeks and cost the whole point.
- **Cold handles finish early.** `object()` completes the full TOC scan before answering; a batch knows its request set, so the scan stops at the last hit and most of a large table is never read when the requests sit near the front. The rows the scan did pass still land in the TOC as always.
- **Misses are data, not yields.** `next` skips absent hashes; `missing()` reports them after exhaustion — a miss has no file position, so it has no place in a file-order yield sequence.
- **One open object at a time**, the same lending shape as `Objects` and for the same borrow reason.

This earns its keep once `view()`/`read()` (#208, #209) exist — descriptors alone are answered by the TOC without seeking; it is batch *body* reads where the monotonic schedule pays.

- [ ] A batch on a cold handle resolves during one forward scan that stops at the last hit (attested with a read/seek-counting wrapper)
- [ ] A batch on a warm handle visits entries in offset order, forward seeks only
- [ ] `missing()` is exact after exhaustion; duplicate request hashes resolve once
- [ ] Corpus: a batch of sampled hashes opens the same objects as per-hash `object()` calls
- [ ] `cargo fmt`, `clippy --all-targets`, `doc --no-deps` clean
