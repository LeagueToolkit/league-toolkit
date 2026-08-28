# ltk_ritobin span model: keep absolute offsets, anchor nodes to tokens

A review of how `ltk_ritobin` represents source positions, answering "should we switch
to relative spans or width/offset spans?", and specifying the model the crate should
move to for incremental parsing. Companion to
[`ltk-meta-meta-generic-options.md`](ltk-meta-meta-generic-options.md), whose option A
(a spanned IR inside ritobin) this interacts with.

> **This is the reference document, not the build plan.** It holds the analysis,
> the rejected alternatives and the benchmark readings, including work that is
> measured and not planned.
> [`ltk-ritobin-incremental-cst.md`](ltk-ritobin-incremental-cst.md) is the
> specification of what actually ships: the API, the step order and the
> acceptance gate per step. Read that one to build. Read this one for the reason
> behind any line of it.

Details:

- Written: 2026-08-24. Updated 2026-08-27: section 4.6 added (folding the range
  types), section 5.4 added (persistent identity above the splice), section 7
  marked resolved by #188, section 9 added (the model as a compact API surface),
  detail 1 of 4.6 now names the stale-range producer (`push_errors`), section 10
  added (implementation plan). Updated 2026-08-28 from a measurement pass: section
  2 corrects property 2, section 4.2 records the span tightening, section 5.2 adds
  the three-array invariant and pins the incremental path to
  `ErrorPropagation::None`, sections 9 and 10 add the `CstBuilder` token-order fix.
  Updated 2026-08-28 from a token measurement pass: section 6.2 added (corpus
  numbers), section 6 reordered behind them, section 3 records why the linear
  delta encoding is still a no, section 9 flags the accessor conflict with the
  array split. Updated 2026-08-28 again after sizing all three arrays: section 6.3
  added (node and children encodings), section 6.4 added (wrapper nodes are 71.6%
  of the node array), section 9.1 added (the accessor decision that gates them),
  section 10 gains step 7 and blocks step 0.3 on 9.1. Updated 2026-08-28 from a
  benchmark pass: section 11 added (the readings, from
  `crates/ltk_ritobin/benches/incremental.rs`), and sections 3, 4.2, 5.1, 5.2, 6,
  6.3 and 6.4 corrected against it. Three claims did not survive. The tail shift
  is slower than this document said, the splice unit is not a root entry, and the
  wrapper-node encoding does not pay in latency. Section 12 added: the goal, the
  ranked work, the closed questions and the gates. Section 11.8 adds a larger
  sample from outside the repository. Section 9.1 is no longer a blocker (12.3).
- Branch: `feat/ptch-resolve`
- Scope: `parse::Span`, `Token`, `Node`, the diagnostics that carry spans, and the
  span payloads in the typecheck IR. The grammar and the CST's flat-vector shape are
  taken as given.

---

## 1. Decision summary

**Section 12 holds the goal and the ranked work.** This section lists the
decisions. Section 12 says which of them matter and in what order, and it closes
the questions that keep returning.

- **Do not switch to relative spans.** They pay off only in structurally shared
  (green/red) trees, which this crate does not have and should not adopt (section 3).
- **Do not switch to offset + width.** At `(u32, u32)` it is bit-for-bit equivalent to
  `start`/`end`, and shrinking the width below `u32` cannot represent the inverted
  spans error recovery produces today (section 3). A token length is a different
  quantity, and section 6 does pack that.
- **Token array space is a separate question, and it is measured.** Section 6 ranks
  the options and section 6.2 holds the numbers. The largest item is the removal of
  the `Newline` tokens, not the packing of fields.
- **The token array is the smallest of the three.** `nodes` is 48% of the CST,
  `children` is 29%, and `tokens` is 23% (6.3). The largest single number in the
  document is that 71.6% of nodes wrap one child (6.4).
- **The accessor decision is an API question, not a gate.** Reference-returning
  accessors freeze the layout, and values with iterators unlock every option in
  6.3 and 6.4 for one mechanical migration. Deciding early still costs one
  migration instead of two. But the work that moves the keystroke tick runs on
  today's accessors, so nothing waits on the answer (9.1, 12.3).
- **Do change where node spans live.** Nodes should stop storing byte spans and store
  their token range instead; the token vector becomes the single source of truth for
  positions (section 4). This is the change that serves both "hyper optimized" and
  "incremental" at once.
- **Incremental parsing is entry-level reparse plus a tail shift**, the
  tree-sitter/Zig school, not the rowan/Roslyn school (section 5). The splice
  unit is an item of the top-level `entries` map, not a root entry. One root
  entry holds 97% to 100% of the file, so a root-level splice measures no gain
  at all (11.3).
- **The tail shift is the larger half of the splice tick.** It runs at 2.46 ms
  per million tokens, not the "well under a millisecond" this document claimed,
  and it is 60% to 82% of the predicted keystroke tick on the two largest samples
  (11.1, 11.8). That makes the array work in 6.3 the main lever on keystroke
  latency, and not only a memory saving. The share rises as the splice granularity
  improves, because smaller entries shorten the reparse and leave the shift
  unchanged.
- **Every number here now has a benchmark.** `benches/incremental.rs` holds the
  five groups, and section 11 holds the readings.

---

## 2. The model today

`Span { start: u32, end: u32 }` (`parse/span.rs:4`), absolute byte offsets into the
source, 8 bytes, `Copy`. It is stored in four places:

| Where | Layout | Notes |
|---|---|---|
| `Token` (`tokenizer.rs:117`) | `kind: u8` + `Span` = 12 bytes, 3 of them padding | the bulk: the corpus runs 6.7-10.5 source bytes per token, so the array is 1.14-1.78x the source size (6.2) |
| `Node` (`cst/tree.rs:121`) | `Span` is 8 of the node's 28 bytes | maintained during `build_tree` with the `start_known` min/max bookkeeping (`parser.rs:127-163`) |
| `parse::Error`, `Diagnostic`, `DiagnosticWithSpan` | one or more `Span`s each | the user-facing surface |
| typecheck IR | `PropertyValueEnum<Span>` via `M = Span` | construction-time only; being redesigned separately (options doc, option A) |

Consumers slice the source directly: `&text[span]` via the `Index` impls, used
throughout `typecheck/state.rs`, `typecheck/walk.rs`, and the debug printer.

Two properties worth naming because the target model leans on them:

1. **Tokens are in strict source order**, and the tokens consumed between a node's
   `Open` and `Close` events form one contiguous run of the token vector. A node's
   extent is therefore fully described by a token range.
2. **A node's byte span is derivable data.** The `Node.span` field should hold
   `tokens[first].span.start .. tokens[last].span.end` for its first and last token
   descendants. It does not hold that value today. `build_tree` seeds a node at
   `Open` with `Span::new(last_span.end, 0)`, and the `Close` path only applies
   `min()` against that seed, so the seed acts as a floor that nothing raises. A
   node that advances a token itself gets the tight start, because the
   `!start_known` path overwrites instead of taking a minimum. A node whose first
   child is a subtree keeps the seed, so its span absorbs the trivia between the
   previous token and its own first token.

   Measured on a small realistic file, 11 of the 55 nodes that own tokens disagree
   with their token-derived span. The affected kinds are `Entry` and `EntryValue`.
   An `Entry` span starts at the end of the previous entry, so it covers the
   newline and the indentation in front of its key.

   ```text
   EntryValue: stored  25..32  " \"PROP\""   derived  26..32  "\"PROP\""
   EntryValue: stored  47..49  " 3"          derived  48..49  "3"
   Entry:      stored 105..181 "\n    \"a\" = B {...}\n"
               derived 110..181 "\"a\" = B {...}"
   ```

   This reaches users. `typecheck/walk.rs` passes `tree.span` to `default_span` for
   diagnostics, and `typecheck/resolve.rs` stores `value.span` and `class.span` as
   IR meta. The `assert_spans_nested` test does not catch it, because
   parent-covers-child still holds. Section 4.2 therefore changes span values, not
   only the place they live.

---

## 3. Rejected alternatives, briefly

**Offset + width.** Same size, same operations, one subtraction moved from `len()` to
`end()`. The only version with any payoff shrinks the width below `u32`, and that
fails on semantics: recovered trees produce inverted spans today
(`typecheck/ir.rs:63` handles a value that starts before its own key;
`Span::is_empty` tolerates `end < start`), and a width encoding cannot represent an
inverted span at all. Every recovery path would need fixing first, for no gain.

**Correction.** An earlier version of that paragraph also claimed that a `u16`
width fails on the data, because string and comment tokens can exceed 64K. The
corpus does not support the claim. The longest token anywhere is 640 bytes (6.2),
so the claim is withdrawn. The verdict does not change, because it rests on the
inverted spans alone.

Note the scope difference this exposes. `Span` must keep `start` and `end`, because
errors and diagnostics use it and both produce inverted spans. A token length is a
different quantity, always non-negative and bounded, so section 6 item 3 packs one
into 16 or 27 bits without touching `Span`.

**Parent-relative or width-only spans (rowan/Roslyn).** That model exists to make
subtrees position-independent so an unchanged green subtree can be shared by
reference across edits. The CST here is a flat arena: `u32` ids into `nodes` /
`children` / `tokens` vectors, no `Arc`, no sharing. Reusing a subtree means copying
vector ranges regardless, so relocatability buys nothing, while every diagnostic and
every `&text[span]` slice would pay O(depth) resolution walking ancestors. Relative
spans in a flat tree is the worst quadrant: rowan's costs without rowan's benefit.
The flat design itself is the right call for this grammar and should stay.

**Token-gap encoding, the linear form of relative.** The paragraph above rejects
parent-relative spans in a tree, whose cost is an O(depth) walk. A linear form
exists and that rejection does not cover it. Store each token as a gap from the
previous token's end plus a length, then keep an absolute checkpoint every 64
tokens. Section 6.2 shows this fits in about 4 bytes per token, because no gap in
the corpus is longer than 50 bytes. It also makes an edit change exactly one gap
field, so the section 5.2 tail shift disappears for the token array.

Reject it anyway, for two reasons. A span query becomes a checkpoint read plus a
scan of up to 64 entries, and the typecheck reads a span per property forever.
The second reason is now measured rather than asserted, and it got weaker before
it got stronger. Section 11.1 puts the whole tail shift at 1.38 ms on skin38, and
the token array is only 20% of that work. Gap encoding therefore removes about
0.28 ms of a 2.30 ms keystroke tick, or 12%, and it charges a 64-entry scan for
every span query in exchange. That is the same worst quadrant, so the reason is
on record and the question does not need to open again.

An earlier version of this paragraph said that section 5.2 "already measures" the
shift. Section 5.2 estimated it and this paragraph repeated the estimate as a
measurement. Section 11.1 is the measurement.

---

## 4. The target model: token-anchored node spans

### 4.1 Layout

`Span` and `Token` keep absolute `start`/`end` exactly as they are. `Node` drops its
`span` field and gains a token range:

```rust
pub struct TokenRange {
    /// First token of this node's contiguous run.
    /// For an empty node, the index the run would have started at (the anchor).
    start: u32,
    len: u32,
}

pub struct Node {
    pub kind: Kind,
    pub tokens: TokenRange,   // replaces `span: Span`
    pub children: ChildRange,
    pub errors: ErrorRange,
}
```

Node size is unchanged (a `TokenRange` is 8 bytes like the `Span` it replaces), but
the node now carries strictly more information: byte positions remain available, and
token-level addressing (which the incremental splicer needs, section 5) comes for
free. The struct above shows the layout; section 4.6 folds it with the existing
`ChildRange`/`ErrorRange` into a single generic definition.

### 4.2 Span resolution

`Node::span` becomes a method taking the tree, mirroring how `ChildRange::get`
already works:

```rust
impl Node {
    pub fn span(&self, cst: &Cst) -> Span {
        match self.tokens.len {
            0 => {
                // empty node: an empty span at the anchor position
                let at = cst.tokens.get(self.tokens.start as usize)
                    .map_or_else(|| cst.source_len, |t| t.span.start);
                Span::new(at, at)
            }
            n => {
                let start = self.tokens.start as usize;
                let first = &cst.tokens[start];
                let last = &cst.tokens[start + n as usize - 1];
                Span::new(first.span.start, last.span.end)
            }
        }
    }
}
```

Two array reads, no walking. Every current consumer of `node.span` already has the
`Cst` in hand (`VisitCtx` carries it; `open_brace_span` already takes it), so the
migration is mechanical: `node.span` becomes `node.span(cst)`. Tokens, parse errors,
and diagnostics keep materialized `Span`s; only nodes change.

The call sites migrate mechanically, but the values change. `span(cst)` returns the
tight token-derived span, so it drops the leading trivia that section 2 measures on
about 20% of nodes. Diagnostics move by a few characters and the `insta` snapshots
churn. Step 4 must state this in its acceptance criteria, and it must not inherit
the bit-identical-output invariant from the parser-perf PR.

**The read costs more, and the amount is known (11.2).** A stored `Span` is one
field of a row the walk already holds. The derived span adds two loads into the
token array, which measures 2.2x to 3.6x the stored read in walk order and 2.5x
to 4.1x in random order. In absolute terms the walk-order cost is 0.47 ns against
1.67 ns per node on skin38. `build_bin` visits 501,572 nodes there in about 30 ms,
so a span read per node adds about 0.6 ms, or 2%. The change is affordable, but it
is not free, and step 4 must carry a `build_bin` non-regression gate to prove it
(section 10). Random order is the LSP pattern, and it pays the 4x, which is one
more reason 5.4 caches a resolved path rather than re-resolving per query.

### 4.3 Empty nodes

Nodes that consume no tokens (empty error trees, the trees behind the
`cur_node.span.end == 0` fixup in `build_tree`) store `len: 0` with `start` set to
the index of the token that would have come next. Their span resolves to an empty
span at that token's start, or at end-of-source when the anchor is one past the last
token. This replaces the current convention (empty span at the previous token's end)
with one that cannot go stale, and it handles EOF uniformly.

### 4.4 What it deletes

The entire span-maintenance layer in `build_tree` (`parser.rs`):

- the `start_known` flag on `StackItem` and its propagation on `Close`,
- the parent-span widening (`min`/`max` on every `Close`),
- the `last_span` threading and the empty-tree `end = start` fixup.

The builder instead records, per stack item, the token index at `Open` and the count
at `Close` - two integer assignments, no branches per token. Node spans can no longer
disagree with token spans because they no longer exist as stored data.

### 4.5 Why this is the optimization that matters

- **One source of truth.** After any mutation of the token stream (incremental relex,
  section 5), node positions are automatically correct. There is no second pass over
  nodes to fix byte offsets and no class of "stale node span" bugs.
- **The bookkeeping is off the hot path.** Span widening ran on every `Close`; the
  token range is two writes per node.
- **It composes with the IR redesign.** Once positions are addressable by token id,
  the spanned IR from option A can carry a 4-byte `TokenId`/`NodeId` as its meta
  instead of an 8-byte materialized `Span`, resolved lazily against the CST. That
  halves the IR's position payload and makes it impossible for IR spans to drift from
  the tree. Diagnostics are the exception: they outlive the tree (they are handed to
  the caller of `build_bin`), so they keep materialized `Span`s, resolved at emission
  time.

### 4.6 One definition for the three ranges

`TokenRange` as drawn in 4.1 is the third copy of a shape the CST already has twice:
`ChildRange` and `ErrorRange` (`cst/ids.rs:49,67`) are both `{ start: u32, len: u32 }`
with an `empty()` and a `get(&Cst) -> &[T]`. Rather than adding the third, fold all
of them into one marker-generic struct:

```rust
pub struct IdxRange<T> {
    start: u32,
    len: u32,
    // zero-sized; `fn() -> T` keeps the type `Copy + Send + Sync` for any `T`
    _marker: PhantomData<fn() -> T>,
}

pub type TokenRange = IdxRange<Token>;
pub type ChildRange = IdxRange<Child>;
pub type ErrorRange = IdxRange<Error>;

impl<T> IdxRange<T> {
    pub fn empty() -> Self;
    pub fn empty_at(anchor: u32) -> Self;   // the 4.3 anchor form
    pub fn len(&self) -> u32;
    pub fn is_empty(&self) -> bool;
}

impl<T> Index<IdxRange<T>> for [T] {
    // start as usize .. start as usize + len as usize (see detail 4)
}
```

Why this shape and not the two obvious alternatives:

- **Not one concrete type.** The marker is what keeps `cst.errors[node.children]` a
  type error. A single untyped range would make every range interchangeable with
  every vector, which is the reason three names exist at all.
- **Not a trait.** A trait cannot reach fields, so each type would still hand-write
  its accessors, and every call site would need the trait in scope - the friction the
  value API review filed against `PropertyExt` (Q5; M-ESSENTIAL-FN-INHERENT).
  Inherent methods on the generic struct need neither. The sharing wanted here is
  implementation sharing, and the generic gives it directly.

The `Index` impl matches the pattern `cst/ids.rs` already uses per id
(`Index<NodeId> for [Node]`, `Index<TokenId> for [Token]`), so both hand-written
`get` bodies go away and `range.get(cst)` becomes `&cst.children[node.children]`.
la-arena's `IdxRange<T>` (rust-analyzer) is this exact design, so there is precedent
for the shape.

Instantiation-specific API keeps a home, because the defining crate can write
inherent impls on a concrete instantiation: `impl IdxRange<Token>` carries
`first()`/`last()`/`ids()` for the span resolution in 4.2, and the 4.3 anchor
contract is documented there. That contract is the one real difference between the
aliases - an empty `TokenRange` keeps a meaningful `start`, an empty `ChildRange`'s
start is a dummy - and it is a difference in contract, not layout, which is exactly
what alias-level docs plus specialized impls express.

Four details decided here so the fold does not re-litigate them:

1. **Out-of-bounds is a panic, uniformly.** `ErrorRange::get` today clamps
   (`if start > cst.errors.len() { return &[] }`), added in `fb4ff3a` as a band-aid
   when the `ErrorPropagation` default flipped to `None` and a stale range sliced
   past the vector. The clamp is partial anyway: a range straddling the end still
   panics, and one fully past the end silently vanishes. The generic `Index` panics
   like slices do; the stale-range producer gets fixed instead of hidden. (The
   producer was identified: `push_errors` computed both `start` and `end` from
   `children.len()` instead of `errors.len()`, so every node-attached `ErrorRange`
   was `len: 0` with a `start` borrowed from the wrong vector. Nobody noticed
   because consumers read the flat `cst.errors` vector directly. **Fixed on
   `fix/ritobin-node-error-ranges` in `d293dfa`, with a regression test. The fix
   is not on `main` yet**, so step 0.1 in section 10 is now a PR of that commit
   rather than new work.)
2. **Serde.** Derive would demand `T: Serialize` even though `T` is phantom;
   `#[serde(skip)]` on the marker plus `#[serde(bound = "")]` keeps today's
   Serialize-only behavior. The wire shape (`{start, len}`) is unchanged.
3. **Debug.** Hand-written (`start..start+len`, or the struct form without the
   marker) so debug dumps and snapshots do not grow `PhantomData` noise.
4. **Range arithmetic widens before it adds.** The fields stay `u32` (the memory
   argument for that is settled: element size is bandwidth in every hot pass here),
   but `start + len` is computed as `start as usize + len as usize`, never in
   `u32`, so a near-cap range cannot wrap. With the fields private, the only code
   doing this arithmetic is the `Index` impl and `Node::span` (4.2), which keeps
   the rule a two-place concern instead of a convention.

`cst/ids.rs` is untouched by #188, by `feat/ptch-resolve`, and by PR #185, so the
fold rides with the rest of section 4 at no coordination cost.

---

## 5. Incremental parsing: entry-level reparse plus tail shift

### 5.1 What the format gives us

Ritobin is unusually friendly to the flat-tree school of incrementality:

- **The file is a keyed list of items, one level below the root.** An earlier
  version of this bullet said the file is a flat list of root entries and that an
  edit is almost always contained in one of them. The first half is true and the
  second half is useless. A ritobin file has 1 to 5 root entries, because
  `entries: map[hash, embed] = { ... }` is one entry that holds the whole payload.
  Measured across the corpus, the largest root entry covers 97.4% to 100.0% of the
  file, and one sample has a single root entry (11.3). A splice at root
  granularity therefore reparses the file and measures no gain.

  The unit that works is one item of that top-level `entries` block. The corpus
  holds 2 to 144 of them per file. The damaged region is found by binary search
  over that block's children, not over the root's children. Everything else in
  5.2 is unchanged.
- **No content token crosses a line break.** Strings become `UnterminatedString` at a
  newline, comments end at the newline. Only the synthetic `Newline` token spans line
  breaks, and it is regenerated from the whitespace run.
- **The lexer carries exactly one bit of cross-token state.** `ends_line` is the
  only place in `lex` that reads outside the run it is scanning, and it reads the
  predicate `ends_value(kind)` over the previous token. `Cursor` holds a byte
  position and nothing else. Relexing can restart at any token boundary given that
  one bit. This is a property of the lexer rather than of the format, so the
  incremental spec makes it an explicit `LexState` type with a resync check and a
  property test.

### 5.2 The algorithm

For an edit replacing byte range `[a, b)` with new text of length `n`
(`delta = n - (b - a)`):

1. **Damage location.** Binary search the top-level `entries` block's children for
   the items whose token ranges touch `[a, b)`, then extend to the previous token
   boundary on the left. Search that block and not the root, for the reason in
   5.1. Reaching the block is a fixed descent of three nodes, so it needs no
   search of its own.
2. **Relex the damaged region.** Restart the lexer at that boundary with the previous
   token's kind as state. Stop when a produced token's boundaries realign with the
   old stream past the edit (standard incremental relex resynchronization).
3. **Reparse the damaged entries** with the existing recursive-descent `impls`,
   producing a small event stream, built into replacement node/child/token ranges.
4. **Splice** the vectors: replace the damaged token run and the damaged entries'
   node/child/error ranges.
5. **Shift the tails.** Everything after the splice point moves by a constant:
   token `start`/`end` by `delta` bytes; `TokenRange.start`, `ChildRange.start`,
   `ErrorRange.start`, and child `NodeId`/`TokenId`s by the respective element-count
   deltas. Each is a linear add of a constant over a dense `u32` array, which is
   an auto-vectorizable pass that runs at memory bandwidth.

   **Measured, and slower than this document claimed (11.1).** The three arrays
   together shift in 1.38 ms on skin38, which is 2.46 ms per million tokens. The
   earlier text said "well under a millisecond for a million-token tail". That was
   an estimate, and it is wrong by about 2.5x. The cost also grows faster than the
   token count, from 0.84 ms per million tokens on the smallest sample to 2.46 ms
   on the largest, because the working set leaves the cache.

   Two consequences. Absolute offsets stay the right call, because a memcpy-class
   pass of 1.4 ms is still far below the cost of resolving relative spans on every
   query forever. But the shift is not a rounding error in the splice tick. It is
   60% of the predicted tick on skin38 and 82% on the larger sample in 11.8, and
   it rises as the splice unit gets smaller. The `nodes` array is the most
   expensive of the three arrays to shift, because three scattered `u32` writes per
   28-byte row scan worse than a dense array. Shrinking `Node` and `Child` (6.3)
   therefore cuts keystroke latency directly, which is a stronger reason for that
   work than the memory saving it was filed under.

Note that step 5's id-shifting exists in any flat-arena splice, span model
regardless. The token-anchored model means the byte-offset shift touches only the
token vector (plus stored error/diagnostic spans, which are few); node positions need
no fixing at all.

**The splice needs one invariant in three arrays.** A subtree occupies one
contiguous run of `nodes`, one of `children`, and one of `tokens`. `nodes` gets its
ids at `Open`, which is pre-order, so a subtree is a run that starts at its root.
`children` flushes its slices at `Close`, which is post-order, so a subtree is a run
that ends with the root's own slice. `tokens` is lexer order.

A check over every node of a sample file confirms all three runs. `Cst::verify`
should assert this, because steps 1 and 4 are a splice only while it holds.

**The incremental path must use `ErrorPropagation::None`.** `Cst::parse` uses `Move`
today. `Move` appends every child error into the parent frame, so the root holds the
whole error list and no node below it holds anything. A damaged entry then has no
handle on the errors it produced, and the splice has to find them inside one
root-owned block. Under `None` each error stays on the node that raised it, so it
splices out with the damaged subtree like the other three arrays. Derive the
root-accumulated view on demand instead of storing it.

`Clone` compounds the same problem and also multiplies storage. A three-line file
with two errors and a nesting depth of four produces eight entries, because every
level copies the errors of its descendants. The growth is O(depth) per error.
`Clone` is opt-in, so this is a note, not a fix.

### 5.3 Staged roadmap

Do not build all of this up front. Each stage is shippable and the benches
(`benches/parse.rs`, `e2e.rs`) decide whether the next one is worth it:

1. **Stage 0 - measure.** Full reparse throughput on real corpus files. The lexer is
   a straight byte scan and the parser allocates up front; sub-frame full reparse for
   typical files is likely already true, and if it holds for the target file sizes,
   incrementality is a non-goal.
2. **Stage 1 - incremental relex, full reparse.** Steps 1-2 plus rebuilding the tree
   from the patched token vector. Lexing dominates in grammars this simple, so this
   captures most of the win with a fraction of the machinery.
3. **Stage 2 - entry-level reparse and splice.** The full algorithm. Only if stage 1
   is measurably insufficient at real file sizes.

**Stage 0 executed (2026-08-27, dev machine, release build, bench corpus in
`crates/ltk_ritobin/samples/`).** Two of this section's bets are now settled by
measurement:

- *Full reparse is NOT sub-frame at real file sizes.* `Cst::parse` runs at
  ~50-90 MiB/s: aatrox (62 KB) 0.7 ms, big (1.3 MB) 21 ms, zaahen (2.2 MB)
  39 ms, skin38 (3.6 MB) 73 ms - plus `build_bin` at ~120-180 MiB/s on top
  (9/16/30 ms respectively). A keystroke tick on a multi-MB file costs
  ~30-100 ms, which is why the LSP debounces today. Incrementality is a real
  goal, not a non-goal.
- *Stage 1's premise is false: lexing does not dominate.* `tokenizer::lex`
  alone runs at ~1.0-1.1 GB/s and is only 5-7% of `Cst::parse` across the
  corpus. The other ~95% is event generation plus `build_tree`. Incremental
  relex with a full reparse would capture almost nothing; skip stage 1 as
  scoped.

Consequences for the roadmap: first profile the parser itself - a 20x gap
between the lexer and the full parse in the same pipeline suggests
constant-factor headroom (event vector, per-node `SmallVec` spills) worth
taking before any splice machinery; then stage 2 (entry-level splice) is the
real keystroke-latency win, making the tick proportional to the damaged entry
instead of the file. The typechecker extension below is not optional in that
world: at 9-30 ms, `build_bin` alone blows the frame budget on big files even
if reparse becomes free.

The typechecker extends the same way when needed: entries are independent apart from
the four root entries and shadow detection, so `build_bin` can recheck only damaged
entries and patch the `Bin`. Out of scope here; noted so the span model is not the
blocker.

### 5.4 Persistent identity above the splice

The splice makes every positional handle volatile: `NodeId`, `TokenId` and
`TokenRange` are arena indices, and step 5 renumbers all three past the splice
point. That is the correct trade - it is what makes them 4 bytes and the shift a
memcpy-class pass - so persistence across edits is never a property of these
types. It lives in a layer above them, re-derived per revision. Every mature
implementation lands here: rust-analyzer persists no syntax nodes across
reparses (salsa keys on `AstId`, re-resolved into the current tree each
revision), Roslyn persists by reusing green nodes, tree-sitter by reusing
subtrees.

Three needs travel under the name "persistent id" and wire up differently:

1. **Identity across edits** - "this is the same entry as last revision", for
   memoized check results, editor anchors, diagnostics reconciliation.
2. **Content addressing** - "this subtree equals that one", for early cutoff.
3. **Position mapping** - "where did offset X move". Already solved by step 5's
   delta; needs no ids at all.

**Identity: the format already provides the keys.** For a general-purpose
language this is the hard part (tree matching, heuristics). Ritobin skips most
of it, because bins are a keyed tree: root entries are addressed by path hash,
properties by name hash. A semantic path - entry path hash, then property name
hashes, then container indices - is a persistent id, stable under any edit that
does not touch the node itself. The PTCH work's `path` module already models
exactly this currency for property patches; the editor anchor and the memo key
should be that type, not a parallel invention. The residue needing synthetic
ids is duplicates (the CST preserves shadowed entries), container items, and
trivia; the standard answer is rust-analyzer's `AstIdMap` pattern - an ordinal
per (kind, occurrence index), best-effort stable across edits that do not
reorder same-kind siblings.

**Maintenance rides the splice.** The identity table maps positionless keys
(semantic paths, ordinals) to current `NodeId`s. Its values are the same kind
of data step 5 already shifts: rebuild the mappings for damaged entries by
re-walking them, add the count delta to the rest in the same vectorizable pass.

**Content hashes: fingerprints yes, storage no.** Two sound uses. First, early
cutoff per entry - the memo key behind 5.3's "recheck only damaged entries". A
`TokenRange` makes the fingerprint nearly free: an entry's content is one
contiguous slice of the source (or of the token vector via the 4.6 fold),
hashed at memory bandwidth with no tree walk. One caveat from the salsa school:
identical bytes are a valid cutoff only if the environment is in the key - an
entry's meaning depends on the root `type:`/`version:` entries and on
shadowing, so the key is (content hash, context), never the hash alone. Second,
optionally, move detection: a hash lets the splice recognize a cut-pasted entry
and carry its cached results to the new position; cheap to add later, skip
until wanted. What is structurally out is hash-consing - Merkle-shared
subtrees, Roslyn's deduplicated green cache, the Unison end of the spectrum.
Sharing requires position-independent subtrees, and token-anchored nodes carry
tree-global token indices on purpose. This is the same trade section 3 made
when rejecting rowan: flat-arena speed and one source of truth, bought by
giving up structural sharing. Content hashes here are fingerprints of
subtrees, never addresses of shared ones.

**The resulting tower.**

```text
semantic path (the PTCH path type)   survives everything but edits to the node itself
  |  AstId side table
ordinal / AstId                      survives edits elsewhere in the file
  |  per-revision map, splice-maintained
NodeId                               one Cst revision
  |  node.tokens: TokenRange
TokenId                              one Cst revision
  |  token.span
Span                                 bytes, materialized only at the leaves
```

Each level re-derives from the one below; only the top two may be held across
an edit tick. Two concrete asks fall out for the API work: stamp `Cst` with a
revision counter, so a stale `NodeId` or `NodePath` is a checkable bug instead
of silent garbage, and specify #176's `NodePath` as per-revision, with the
semantic path as the cross-revision currency. AST-to-CST provenance joins this
tower at the `NodeId` level, and an eventual salsa layer keys at the `AstId`
level, as rust-analyzer does.

---

## 6. Benchmark-gated micro work

Section 6.2 holds the measurements that size this list. Work the items in this
order, and bench each one before the next.

1. **Remove the `Newline` tokens and keep a `starts_line` bit.** `Newline` is a
   parser-internal signal that only `parse/impls.rs` reads. The printer and the
   typecheck never touch it, and the tokenizer already collapses a whitespace run
   into one synthetic token. Moving the signal to a spare bit on the next token
   removes 10.5% to 15.8% of the tokens, their `Advance` events, and their `Child`
   entries. By the perf doc ratios that is about a 10% cut across four arrays and
   the event stream at once. Fewer elements beats smaller elements, so this ranks
   first.
2. **Split the token array into one array per field.** `Parser::nth` reads
   `it.kind` and nothing else, and it drives the 31 ms event-generation phase.
   Today one kind read pulls 12 bytes through the cache. With a separate `kinds`
   array the skin38 lookahead scan streams 560 KB instead of striding 6.5 MB.
   After step 4 the replay loop reads no spans either, so the parse phase touches
   kinds alone. The split also reclaims the 3 padding bytes for free.
3. **Pack the remaining fields.** `{ start: u32, len: u16, kind: u8 }` needs a side
   table for the 1-3 tokens per file that are longer than 255 bytes.
   `{ start: u32, packed: u32 }` with `len` in 27 bits and `kind` in 5 bits needs
   no escape for a file under 128 MB. Drop the `len == MAX` relex escape this
   section proposed before. The longest token in the corpus is 640 bytes, so that
   escape is dead code.

Combined on skin38: 6,562 KB today, about 4,400 KB after items 1 and 2, about
3,400 KB after all three.

**Where the `starts_line` bit goes.** `TokenKind` has 25 variants, so 5 bits hold a
kind and 3 bits stay free in the `kinds` byte of item 2. Put the flag in one of
them. It costs no memory, it adds no second cache stream, and `Parser::nth` already
loads that byte. The read cost is one AND on the hot path.

A standalone bitset is the alternative. It is 0.125 bytes per token, which is 1.04%
of the array and 61 KB for skin38, and a splice shifts 7,826 words of tail for that
file. Neither number is a problem. What it buys is a word-at-a-time scan for line
starts, and nothing scans line starts in bulk today, so prefer the spare bit and
keep the bitset in reserve.

An LSP line index is a third structure and answers a different query. Byte offset
to line number wants `line_starts: Vec<u32>`, which is 313 KB for skin38 and
answers in O(log n). Materialize that on demand. Do not confuse it with the parser
flag, which answers "does this token start a line" in O(1).

`Token` and `Span` derive `serde` today, so every item above changes the `Cst` wire
format. This is the same compatibility concern the value API review raised (finding
A2). Gate the whole list on it.

**Not proposed: interning or dedup of the token array.** The array holds
`(kind, span)` and no text. Spans are unique by construction, because they ascend
strictly, so there is nothing to dedup. Interning pays off on the hash and name
strings in the typecheck IR instead, which is the options doc's ground.

**Corrected.** An earlier version of this section also ruled out splitting spans
out of `Token`, because the flat layout is already the cache-friendly shape. That
holds for span consumers and fails for the parser, so item 2 now stands. Prove it
with a prototype rather than by argument.

**The memory-bound premise now has support (11.4).** This list and the perf doc
both gate on the pipeline being memory-bound, and neither defined the experiment.
The `entry_reparse/whole_file` readings answer it. `Cst::parse` runs at 121 MiB/s
on a 23 KB file and 51 MiB/s on a 3.7 MB file, a 2.4x decline while the CST
working set grows from 163 KB to 29 MB. A compute-bound pipeline holds its rate
across that range. This one does not, so smaller elements should pay.

One confound remains, and it is worth naming because it is cheap to remove. The
`Vec` doubling reallocs of 6.1 also grow with file size, so they could produce the
same curve. The parser-perf PR pre-sizes those vectors. Rerunning this group after
that PR separates the two causes, and it is the gate this list should use.

### 6.1 The event pipeline is measurably memory-bound (2026-08-27)

The stage-0 profile (5.3) was split further: for skin38 (3.6 MB), `Cst::parse`
is lex 3.6 ms / event-gen (`impls::file`) 31 ms / `build_tree` 31 ms. The
gating evidence for this section now exists, and it points at the event
stream, not the token vector: `Event` is **40 bytes** (the `Error` variant's
inline `ErrorKind` + `Option<Span>` payload sets the size for all variants),
and skin38 produces 1.56M events = **62.5 MB**, written once by event-gen and
read once by `build_tree` - 17x the source text, grown from `Vec::new()`
through doublings, then rescanned a third time by the `nodes_len` pre-count.
Corpus ratios: ~2.8 events and ~0.9 nodes per token; ~2.1 children per node,
with 15-16% of nodes spilling the `SmallVec<[Child; 4]>` inline buffer
(~77K heap alloc/frees per skin38 parse).

This work is now specified as a standalone PR in
[`ltk-ritobin-parser-perf.md`](ltk-ritobin-parser-perf.md) - it is independent
of this doc's train (no `parse/` overlap with `feat/ptch-resolve` or PR 185)
and lands before step 4, which rebases its `build_tree` rewrite onto it. The
list below is the summary; the proposal doc is authoritative.

Constant-factor work list, in order of expected value; all internal except
that `Event`/`Parser.events` are technically `pub` (doc-warned "do not use"):

1. **Shrink `Event` to 2 bytes.** Move the error payload to a side vec
   consumed in order during replay (`Vec<(ErrorKind, Option<Span>)>`); the
   enum becomes `Open(Kind) | Close | Advance | Error`, niche-packed to 2
   bytes with `Kind` as `repr(u8)`. 62.5 MB of event traffic becomes 3.1 MB.
2. **Pre-size `events`** to `tokens.len() * 3` (measured 2.79) in
   `Parser::new`; kills the realloc doublings.
3. **Count opens in the parser** (a counter bumped in `open`/`scope`/
   `open_before`) instead of re-scanning the event vec in `build_tree`.
4. **Move the token vector wholesale**: `cst.tokens = self.tokens` up front,
   `Advance` advances an index cursor. Kills 560K per-token pushes into a
   zero-capacity vec and the `peekable` wrapper (error-span peeking indexes
   the vec instead).
5. **Replace per-node `SmallVec` buffers with one shared scratch stack**:
   `StackItem` records a frame start into a shared `Vec<Child>`; `Close`
   copies `scratch[start..]` into `cst.children` (order is preserved -
   identical layout to today), truncates, and pushes the `Tree` child onto
   the parent frame. Same for errors. Eliminates the ~77K spill allocations.
   Coordinate with the step-4 node change, which rewrites this loop anyway.
6. **Kill `events.insert` in `open_before`**: `block()` is its only caller,
   post-wrapping `Class` list items; pass the block context into
   `stmt_or_list_item` so the `Class` arm opens the `ListItem` itself.
7. Trivia: `stmt_or_list_item` matches on `(nth(0), nth(1), nth(2))` but no
   arm inspects the third element - drop it.

The step-4 token-anchored node change compounds with this: it deletes the
per-`Advance` span writes and the per-`Close` `split_at_mut` widening from
the replay loop entirely (4.4). Realistic combined target is ~2.5-4x on
`Cst::parse` (skin38 ~66 ms toward ~15-25 ms) - a big quality-of-life win for
the LSP debounce, but still not per-keystroke budget at 4 MB, which stays
stage 2's job. Not proposed here: dropping the event stream for direct arena
building - it would forfeit the mark/retype flexibility error recovery uses,
and is only worth revisiting if this list lands and the profile still points
at the replay.

### 6.2 Token corpus measurements (2026-08-28)

Lexed from `crates/ltk_ritobin/samples/`. These numbers size every item in section
6.

| file | source | tokens | bytes/token | array now |
|---|---|---|---|---|
| aatrox.rito | 61 KB | 5,918 | 10.5 | 69 KB (1.14x source) |
| azirultsoldier.rito | 23 KB | 3,037 | 7.8 | 36 KB (1.54x) |
| big.rito | 1,325 KB | 171,989 | 7.9 | 2,015 KB (1.52x) |
| zaahen.rito | 2,231 KB | 305,061 | 7.5 | 3,575 KB (1.60x) |
| skin38.rito | 3,688 KB | 559,959 | 6.7 | 6,562 KB (1.78x) |

`size_of::<Token>()` is 12. `kind` takes 1 byte and `Span` takes 8, so alignment
adds 3 bytes. That padding is 25% of the array.

Distribution facts, measured across the whole corpus:

- **Token length.** 640 bytes is the maximum. Only 1-3 tokens per file are longer
  than 255 bytes. No token is longer than 65535 bytes. This is what kills the
  `len == MAX` relex escape.
- **Gap between tokens**, which is the whitespace run the lexer skips. 50 bytes is
  the maximum. No gap is longer than 255 bytes, and about 96% of gaps are 15 bytes
  or shorter. This is what makes the section 3 delta encoding feasible on paper.
- **Kinds in use.** 17 at most, so 5 bits hold a kind and 27 bits stay free in a
  packed word.
- **`Newline` tokens.** 10.5% to 15.8% of all tokens, which is item 1 of section 6.
- **Statement boundaries.** `Entry` plus `ListItem` plus `EntryTerminator` nodes
  are 34.9% to 39.8% of the token count. That is how often a parse reads the
  `starts_line` flag, against the several reads per token that `Parser::nth` makes
  of a kind.

### 6.3 The node and children arrays (2026-08-28)

Section 6 and 6.2 both size the token array, and the token array is the smallest of
the three. Measured on the same corpus:

| array | element | skin38 | share | big | aatrox |
|---|---|---|---|---|---|
| `nodes` | 28 B | 13,715 KB | 48% | 4,291 KB | 153 KB |
| `children` | 8 B | 8,293 KB | 29% | 2,570 KB | 90 KB |
| `tokens` | 12 B | 6,562 KB | 23% | 2,015 KB | 69 KB |
| total | | 28,570 KB | | 8,877 KB | 312 KB |

The whole CST is 5.1x to 7.8x the source size, and the ratio grows with the file.
Three structural facts hold on every sample. `children` is exactly
`tokens + nodes - 1`, so every token and every non-root node is one child entry.
Every id needs 20 bits at most, so bit 31 is free in `NodeId` and in `TokenId`.
Tree depth reaches 47, so a depth fits in a `u8`.

Four encodings, ranked by saving against risk:

1. **Pack `Child` into a `u32`.** `enum Child { Token(TokenId), Tree(NodeId) }` is
   8 bytes and carries 33 bits. Put the tag in bit 31 and assert that the id fits.
   `children` drops from 8,293 KB to 4,146 KB on skin38, which is 14% of the whole
   CST, and no traversal changes.
2. **Move `ErrorRange` off `Node` into a side table.** The nodes that carry errors
   are 0 of 5,580, 1 of 156,938, and 0 of 501,572. The field wastes 8 bytes on
   every other node. A sorted `Vec<(NodeId, ErrorRange)>` with a binary search puts
   the cost on a path that almost nothing takes. `Node` drops from 28 bytes to 20,
   which is another 14% of the CST.
3. **Emit `children` in pre-order, then drop the length.** `build_tree` appends a
   node's slice at `Close`, which is post-order, so `ChildRange.start` is not
   monotone in `NodeId` today. Emit in pre-order and `len` becomes
   `start[i + 1] - start[i]`. That is plain CSR adjacency layout. It halves
   `ChildRange` to 4 bytes, about 2 MB on skin38, and no traversal changes.

   **The walk gain is real and the shortcut to it is not (11.5).** A pre-order
   child array walks 1.99x faster than today's close-order array on skin38 and
   1.22x faster on big and zaahen. That is 535 microseconds saved per walk on
   skin38. But reaching that layout with a separate permutation pass costs 1.79 ms
   on the same file, so it needs 3.3 walks to break even there and about 10 walks
   on big and zaahen. A parse plus a `build_bin` plus a print is 2 to 3 walks.
   **A permutation pass loses. Only native pre-order emission from `build_tree`
   wins**, and that means `build_tree` has to learn each node's child count before
   it writes the slice. Price that rewrite before committing to the item.
4. **Derive `children` and delete the array.** Section 4's `TokenRange` makes the
   child list redundant. Given nodes in pre-order, a token range per node, and a
   `subtree_size: u32`, walk `t` across the parent's token range. If `t` equals a
   child's `tokens.start`, emit that child and jump to its `tokens.end`. Otherwise
   emit `Token(t)` and step by one. Direct children come from the pre-order jump
   (`j = i + 1`, then `j += subtree_size(j)`). Empty nodes still place correctly,
   because `empty_at` gives them an anchor. The order is unambiguous because of the
   three-array invariant in 5.2.

Item 4 removes 8.3 MB and replaces `ChildRange` with 4 bytes, so skin38 falls from
28,570 KB to about 9,800 KB, which is 2.65x the source.

**Locality, measured 2026-08-28.** An earlier draft of this section warned that the
merge costs latency. The measurement does not support that warning. A pre-order
walk reads the `nodes` array with a mean absolute jump of 1.00 entries, which is a
linear scan. The same walk reads the `children` array with a mean absolute jump of
42.9 entries on skin38 and 53.0 on big, which is 343 to 424 bytes, and 54% of those
jumps go backward. The cause is that `build_tree` appends each slice at `Close`, so
`children` sits in post-order while every walk runs in pre-order.

The stored array is therefore the scattered one, and the derived merge reads only
arrays that scan linearly. The merge spends more instructions per child and saves
cache misses. Which one wins depends on whether the walk is memory-bound, and 11.4
says it is. Bench against `build_bin` as well as `parse` before choosing, but do
not assume the merge is the slower option.

**The stride now has a price (11.5).** Walking the same tree over a pre-order child
array instead of today's close-order array runs 1.99x faster on skin38. So the
scattered layout costs about half the child-walk time, and the locality argument
above is confirmed rather than merely plausible. That bounds what item 4 can win
from locality alone, because item 3 reaches the same layout without deriving
anything.

An earlier version of this paragraph called item 3 a win on both memory and latency
and the cheapest of the four. The memory half stands. The latency half does not
survive its own measurement, because the only cheap route to the layout is a
permutation pass, and that pass costs more than it saves. Item 3 is now ranked on
its 4-byte `ChildRange` alone.

**A cache does not help a walk.** `build_bin`, the printer and the typecheck each
visit every node exactly once, so a memo table on child lists takes every miss and
no hit. Caching pays only for repeated access to one node, which is the LSP query
pattern, and there the thing worth caching is the resolved path from 5.4 rather
than the child list. Two targeted exceptions still earn their place. Materialize
the root's children, because it is the largest list in the file and the
binary-search target for 5.2 step 1. Write the merge as an iterator that carries
the next child boundary down the walk, so one traversal never rescans.

Free on top of item 4: 25 kinds need 5 bits and 501K nodes need 20, so
`subtree_size: 27 | kind: 5` packs into one `u32` and `Node` reaches 12 bytes.

### 6.4 Wrapper nodes are 71.6% of the node array

The largest single number in this document. Measured across the corpus, the nodes
that hold exactly one child are 71.2% to 73.6% of all nodes. On skin38 that splits
into 45.9% holding one token and 25.7% holding one subtree. The kind histogram
gives the reason: `Literal` is 15% of nodes, and `EntryKey`, `EntryValue`,
`TypeExpr` and `EntryTerminator` are 10% each.

At 28 bytes plus one child entry each, those 359K nodes on skin38 cost about
12.7 MB of the 28.5 MB total. No bit-packing competes with not allocating them.

**This is a memory item only. It costs latency (11.6).** A walk over an id stream
where 46% of the ids decode from the id itself, against a walk over a full node
array, measures 0.77x to 0.99x. It is slower on every sample in the repository
corpus, by 26% on the smallest and by 8% on skin38, and it reaches 1.013x on the
46 MiB CST of 11.8. The compacted array is 54% of the rows, and
that saving does not pay for the bit-31 branch and the extra hop into the token
array. The branch is close to a coin flip at a 46/54 split, which is the worst case
for a predictor.

So the case for 6.4 is 12.7 MB, and the price is up to a quarter of the walk time
on small files. Take the item if memory is the binding constraint. Do not take it
expecting a speedup. Two readings in an earlier draft of this note showed a gain on
the two largest files. Those came from a baseline that read the kind and the span
from two separate arrays, which inflated it. The corrected baseline reads one row,
and the gain disappears.

**Nothing is a wrapper by grammar.** `parse/impls.rs` gives every candidate an
error path that produces a different shape. `Literal` is `scope(Literal, advance)`
in two places, but its `UnterminatedString` arm holds an `ErrorTree` instead.
`EntryKey` and `TypeArg` hold one token, or none when `expect` fails. `EntryValue`
holds one subtree, or a subtree plus newline tokens on its error arms. So the
strategy cannot be "delete the kind". It has to be **virtualize per node**: the
encoder asks whether this node holds exactly one token child, and skips the
allocation when it does. Error paths materialize as they do now. The grammar does
not change at all.

**The encoding.** Spend no `nodes` entry and put the node in the id:

```text
NodeId (u32)
 31    30..26    25..6
 [1]  [ kind ]  [ token id ]     bit 31 set marks a virtual node
```

A kind needs 5 bits and a token id needs 20, so 26 of 32 bits are enough.
`Cst::node(virtual)` builds `Node { kind, tokens: TokenRange::new(tok, 1) }` on
demand, and `Cst::children(virtual)` yields one `Child::Token(tok)`. `Child` keeps
two cases and one tag bit, so it still packs into a `u32` per 6.3 item 1. Every
`Visitor` sees what it sees today.

**Where the encoding stops: wrapper chains.** `EntryValue` holds `Literal` holds a
token, and `ListItem` holds `Literal` holds a token. Both occur in ordinary input.
A virtual node cannot point at another virtual node, because the payload has no
room to nest. Encoding two kinds plus a token needs 32 bits exactly, which breaks
the first time somebody adds a `TokenKind`.

So the finding splits in two, and the halves belong in different places:

- **The 45.9% that hold one token** are an encoding change. Virtual ids handle them
  with no grammar change and no chain problem. On skin38 that is 230K nodes, about
  6.3 MB of the node array plus 1.8 MB of children.
- **The 25.7% that hold one subtree** are a grammar question. `EntryValue` earns
  its place only if `Entry`'s children are not positional. That belongs with the
  AST work.

---

## 7. Small fixes independent of the model

**Resolved.** All three landed on `main` in #188 (`fix!(ltk_ritobin): half-open
spans and balanced visitor unwinding`, merged 2026-08-25): the doc comment now reads
`[start, end)`, `contains` went half-open rather than documented-inclusive, and the
expected-token error span is clamped to the source. Kept for the record; this branch
predates the merge and still shows the old code.

Worth doing whenever the file is next touched:

1. `Span`'s doc comment says "(offset and length)"; the fields are `start`/`end`.
2. `Span::contains` is end-inclusive (`offset <= self.end`) while `intersects` is
   end-exclusive. Inclusive-end is often what an editor wants for cursor-at-token-edge
   queries, but the asymmetry is undocumented - document it, or split into `contains`
   and `contains_inclusive`.
3. The expected-token error path (`parser.rs:179-184`) manufactures a span one past
   the current token, which can point past EOF. `Index<&Span> for str` clamps `end`
   but not `start`, so such a span can panic on slicing. Saturate both at emission.

---

## 8. Sequencing

The node change (section 4) restructures the same code the option A spanned IR work
will touch, and the IR's `TokenId` meta depends on it. Do them together, as one
restructuring of ritobin's position handling, after the PTCH work lands. Nothing in
sections 4-6 blocks or is blocked by anything on this branch; section 7 items are
safe at any time. Section 10 expands this into the concrete PR/issue train.

---

## 9. API proposal

The model above as a surface, in the shape we use for the repo issues: signatures
only, minimal comments. `Span`, `Token`, `NodeId`, `TokenId`, and `Child` are
unchanged. `CstBuilder` is not, for the reason in the migration list below.

```rust
// cst/ids.rs - one definition replaces ChildRange/ErrorRange and adds TokenRange (4.6)
pub struct IdxRange<T> {
    start: u32,
    len: u32,
    _marker: PhantomData<fn() -> T>,   // #[serde(skip)]; Debug hand-written as start..start+len
}

pub type TokenRange = IdxRange<Token>;
pub type ChildRange = IdxRange<Child>;
pub type ErrorRange = IdxRange<Error>;

impl<T> IdxRange<T> {
    pub(crate) fn new(start: u32, len: u32) -> Self;
    pub fn empty() -> Self;
    pub fn empty_at(anchor: u32) -> Self;   // the 4.3 anchor form
    pub fn start(&self) -> u32;
    pub fn len(&self) -> u32;
    pub fn is_empty(&self) -> bool;
}

impl<T> Index<IdxRange<T>> for [T] {
    type Output = [T];   // widens to usize before adding; panics like a slice, no clamp
}
impl<T> IndexMut<IdxRange<T>> for [T];   // the splice's shift pass mutates in place
```

```rust
// cst/ids.rs - the 4.3 anchor contract lives on the Token instantiation
impl IdxRange<Token> {
    /// An empty range keeps a meaningful `start`: the index the run would have started at.
    pub fn first(&self) -> Option<TokenId>;
    pub fn last(&self) -> Option<TokenId>;
    pub fn ids(&self) -> impl Iterator<Item = TokenId>;
}
```

```rust
// cst/tree.rs
pub struct Node {
    pub kind: Kind,
    pub tokens: TokenRange,   // replaces `span: Span`
    pub children: ChildRange,
    pub errors: ErrorRange,
}

impl Node {
    pub fn span(&self, cst: &Cst) -> Span;              // 4.2: two array reads
    pub fn open_brace_span(&self, cst: &Cst) -> Span;   // unchanged
}
```

```rust
// cst/tree.rs - slice accessors pair with the Index impls; `errors` stops being a pub field
impl Cst {
    pub fn nodes(&self) -> &[Node];
    pub fn children(&self) -> &[Child];
    pub fn tokens(&self) -> &[Token];
    pub fn errors(&self) -> &[Error];

    pub fn revision(&self) -> Revision;   // the 5.4 ask
}

pub struct Revision(u64);   // Copy + Eq + Hash; bumped by every splice, 0 for a fresh parse
```

Migration is mechanical:

- `node.span` becomes `node.span(cst)`.
- `range.get(cst)` becomes `&cst.children()[range]` (respectively `tokens()`,
  `errors()`); both hand-written `get` bodies are deleted, the `ErrorRange` clamp
  with them, and `push_errors` computes its range from `errors.len()` (detail 1
  in 4.6).
- `build_tree` records the token index at `Open` and the count at `Close`; the
  `start_known`/span-widening/`last_span` machinery goes (4.4).
- `CstBuilder` must allocate token ids in emission order. Today it breaks the
  source-order invariant that `span(cst)` depends on, in four ways:
  - `block` creates the braces after the children already hold lower ids.
  - `entry_tree` creates the separators after the key, the type, and the value.
  - The string arm uses one `Quote` id at two positions in the tree.
  - `entry_tree` allocates a `Colon` that it drops when there is no type.

  A dump of a builder tree shows the damage: token ids walk
  `[0, 2, 4, 3, 5, 1, 7, ...]`, one `Quote` id appears twice, and one `Colon`
  reaches no node. Until this is fixed, `Node::span(cst)` returns wrong offsets for
  every tree the builder makes. Node spans are already meaningless there, because
  most builder tokens carry `Span::default()`, so this promotes a quiet fault into
  a load-bearing one rather than creating it.

**One open conflict with section 6.** The `tokens()` accessor above returns
`&[Token]`, and `Index<IdxRange<Token>> for [Token]` needs a real slice. Item 2 of
section 6 splits the token array into one array per field, and no slice of `Token`
can back that. Section 9.1 turns this into one decision that covers every layout
option, not a per-trick argument.

### 9.1 The accessor decision and what it actually gates

**Superseded in part by 12.3.** This section reads as a gate on the whole layout
program. Section 11 measured what the gate holds back, and the answer is less than
this section assumed. The three items that move the keystroke tick all work with
today's reference accessors. Only the derived child list and wrapper
virtualization need the value surface, and section 11 leaves one unproven and
rules the other out for speed. So decide this on API-design grounds and not as a
blocker. The argument below stands on its own terms and is still the reason to
prefer values.

The old framing said to decide it before the section 9 issue is filed, because it
is one mechanical migration now and a second breaking release later. That cost is
real and it is still the argument for deciding early.

`trait Visitor` is not the problem and needs no change. `enter_tree(ctx, tree:
NodeId)` and `visit_token(ctx, token: TokenId, parent: NodeId)` hand out ids, never
references, so the walker stays free to enumerate children however the storage
works. Three accessors are the problem, because each one returns a reference into
storage:

```rust
VisitCtx::node(&self, id) -> Option<&Node>   // a reference to a stored row
ChildRange::get(&self, cst) -> &[Child]      // a real slice
Cst::nodes() / children() / tokens() -> &[T] // the section 9 proposal above
```

A `&Node` cannot point at a node that 6.4 never allocated, and a `&[Child]` cannot
exist if 6.3 item 4 derives the children. Keep these three and the layout is frozen
at what it is today. Only `Child` packing (6.3 item 1) and the error side table
(6.3 item 2) stay reachable, which is 28% of the CST and the end of the road.

The alternative surface returns values and iterators:

```rust
/// bit 31 set marks a virtual node: no row in `nodes`, decoded from the id (6.4)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

/// A value, not a stored row. 12 bytes and `Copy` after 6.3 and 6.4.
#[derive(Clone, Copy)]
pub struct Node {
    pub kind: Kind,
    pub tokens: TokenRange,
}

impl Cst {
    pub fn node(&self, id: NodeId) -> Option<Node>;    // by value
    pub fn token(&self, id: TokenId) -> Option<Token>; // by value
    pub fn children(&self, id: NodeId) -> impl Iterator<Item = Child> + '_;
    pub fn errors_of(&self, id: NodeId) -> &[Error];   // the 6.3 side table
    pub fn span(&self, id: NodeId) -> Span;
}
```

Four breaks, and they are the whole cost:

1. `&Node` becomes `Node`. The struct is `Copy` and 12 bytes once `ErrorRange` and
   `ChildRange` leave it, so a return by value often beats the pointer chase it
   replaces.
2. The `children` field becomes the `children(id)` iterator. This is the same break
   that 6.3 item 4 needs, so the two land together.
3. `errors` moves to the side table, which 6.3 item 2 justifies on its own.
4. `nodes()` and `children()` stop returning slices. `tokens()` survives as a slice
   only if section 6 item 2 does not split the token array.

With this surface every option in 6.3 and 6.4 becomes an internal change, and the
consumers never move again.

---

## 10. Implementation plan

**Superseded by [`ltk-ritobin-incremental-cst.md`](ltk-ritobin-incremental-cst.md),
which is the current plan.** That document carries the same train with the API
signatures, the acceptance gate per step and the splice specified. It also ranks
the array work by effect on the keystroke tick rather than by memory saved, per
12.2. This section is kept because it records the sequencing constraints and why
each one exists.

The concrete PR/issue train for sections 4 and 9. The order is forced by call-site
conflicts (each PR rewrites code the next one touches), so everything after step 0
is intentionally serial; PR 185 is deliberately last. Nothing from section 7 needs
tracking (all landed in #188).

**Step 0 - now, all five independent:**

1. Bugfix PR to `main`: `push_errors` computes its range from `errors.len()`, with
   a regression test asserting a node's attached errors round-trip. It must precede
   the fold (detail 1 in 4.6 deletes the clamp that hides it) and is a live bug on
   `main` regardless of the model work. **The work is done in `d293dfa` on
   `fix/ritobin-node-error-ranges`. Only the PR to `main` is outstanding.**
2. Commit the design docs. The `docs/design/*.md` files are untracked; issues need
   permalinks to sections, not quotes.
3. File the tracking issue for the span model. Section 9 is the body: the code
   blocks plus the migration list, linking this doc for rationale. Mark it breaking
   (`Node.span` field removed, `Cst.errors` private, `get` methods deleted).
   **No longer blocked on 9.1**, per 12.3. References or values still changes what
   the issue proposes, and deciding early is still one migration instead of two.
   But the work that moves the keystroke tick does not wait on the answer, so file
   the issue against today's accessors and revisit the surface on its own terms.
4. Draft the #176 additions. They settle vocabulary the later PRs use - `Boundary`,
   `NodePath` as per-revision, the revision counter, NodeId provenance - cheap now,
   expensive to retrofit.
5. Comment on PR 185 with the sequencing and the two hazards heading its way:
   `fine_path_to` silently changes behavior when rebased over half-open `contains`,
   and the node change deletes the `span` field its AST lowering copies.

**Step 1 - land `feat/ptch-resolve`.** Section 8's constraint: the node change
restructures `parser.rs` and the typecheck files the branch is actively editing.

**Step 2 - PR: the `IdxRange<T>` fold (4.6).** `refactor(ritobin)!`: aliases,
`Index`/`IndexMut`, `Cst` slice accessors, `errors` field private, serde/Debug
details, clamp deleted. No node change yet. It waits for step 1 despite
`cst/ids.rs` being untouched because the `get` call sites it migrates live partly
in the typecheck files ptch-resolve edits. After step 1 it is mechanical.

**Step 3 - #175 then #176 implementation**, in the order already agreed. Both are
written against materialized spans as they stand; step 4 migrates `node.span` to
`node.span(cst)` across them in one sweep.

**Step 4 - PR: token-anchored node spans (4.1-4.5).** `feat(ritobin)!`:
`tokens: TokenRange` on `Node`, `span(cst)`, the `build_tree` simplification (4.4),
the `CstBuilder` token-order fix (9), the 4.3 anchor convention, plus three riders
that belong with it: the `Revision` counter on `Cst` (the 5.4 ask),
`ctx.text_of(id)` on `VisitCtx` (the #175 misc note), and `Cst::verify` for the 5.2
invariant. Closes the tracking issue.

Three acceptance notes for this step. Node spans get tighter (4.2), so snapshot
churn is the expected result and not a regression. The `CstBuilder` fix needs its
own test, because the round-trip tests pass today with the token order broken. And
the step must hold the `build_bin` bench group flat, because `span(cst)` costs 2.2x
to 3.6x a stored field read and `build_bin` reads a span per node (4.2, 11.2). The
projection is about 2%, so a regression above 5% means the migration went wrong
somewhere else and needs a look.

**Step 5 - the option A spanned IR with `TokenId` meta**, immediately after step 4
as the coordinated pair section 8 describes. Open dependency: the meta A/B
decision (options doc), which is the long pole to resolve during steps 1-3 -
step 5 cannot be scoped until it is made.

**Step 6 - PR 185 rebases last**, onto the post-step-4 world: NodeId provenance,
the `WalkOutcome` visitor contract, the map-key-primitive rule port.

**Deferred, not tracked yet:** section 6's token packing stays benchmark-gated.

**No longer deferred: the stage 2 splice.** An earlier version of this paragraph
held the incremental stages here, waiting for a measurement. Stage 0 and section
11 supplied it. The splice is the goal this plan serves (12.1 and 12.2), so it
needs a tracking issue of its own, sequenced after step 4 because it depends on
token-anchored nodes. Stage 1 stays cancelled for the reason in 5.3.

**Step 7 - the array work (6.3 and 6.4), only after 9.1 is settled.** Take it in
the order the sections rank it, and bench `build_bin` as well as `parse` at each
step. The two free items are `Child` as a `u32` and the error side table, which
together cut 28% of the CST and change no traversal. Everything past them needs
the value-and-iterator surface from 9.1. The subtree half of 6.4 waits on the AST
discussion, so it cannot start before step 6.
Ranked on latency the order changes, because 11.1 shows the tail shift is 60% of
the splice tick: `Child` as a `u32` and the shorter `Node` cut that shift directly.

---

## 11. Benchmark readings (2026-08-28)

`crates/ltk_ritobin/benches/incremental.rs`, criterion, release, dev machine.
Run it with `cargo bench -p ltk_ritobin --bench incremental`. Every number below
is a median. Corpus is `crates/ltk_ritobin/samples/`.

Two notes on method, because both changed a result.

`Cst` keeps its arena private, so the bench rebuilds an equivalent arena through
the public API. A pre-order walk gives node order and token order. A post-order
walk gives the child order that `build_tree` writes at `Close`. The rebuild runs
in setup and is not timed.

Every A/B reading compares two rows of the same width. An early draft of the bench
read today's span from a dense 8-byte vector and the token range from a 28-byte
row, which measured the row width and not the span model. The same fault inflated
the wrapper-node baseline by splitting one row across two arrays. The bench now
asserts that both row types have the same size.

### 11.1 The tail shift

All three arrays shift in one pass, which is what a splice does. An earlier
scratch measurement timed each array in its own loop and reported about half these
numbers. Separate loops keep each array hot on its own, so they understate a pass
that touches 29 MB at once.

| file | tokens | shift | per 1M tokens |
|---|---|---|---|
| azirultsoldier.rito | 3,037 | 2.56 us | 0.84 ms |
| aatrox.rito | 5,918 | 5.82 us | 0.98 ms |
| big.rito | 171,989 | 208 us | 1.21 ms |
| zaahen.rito | 305,061 | 420 us | 1.38 ms |
| skin38.rito | 559,959 | **1.38 ms** | **2.46 ms** |

The per-token cost triples from the smallest file to the largest, so the pass does
not stay at cache bandwidth. Section 5.2 carries the consequences.

### 11.2 A stored node span against a derived one

Nanoseconds per node. "Walk order" reads nodes by ascending id, which 6.3 measures
as the real traversal order. "Random order" is the diagnostic and LSP pattern.

| file | stored, walk | derived, walk | ratio | stored, random | derived, random | ratio |
|---|---|---|---|---|---|---|
| aatrox.rito | 0.28 | 0.63 | 2.25x | 0.45 | 1.14 | 2.51x |
| azirultsoldier.rito | 0.28 | 0.59 | 2.08x | 0.41 | 1.02 | 2.47x |
| big.rito | 0.41 | 0.95 | 2.35x | 1.05 | 2.92 | 2.78x |
| zaahen.rito | 0.43 | 0.95 | 2.22x | 1.25 | 3.66 | 2.93x |
| skin38.rito | 0.47 | 1.67 | 3.56x | 1.80 | 7.34 | 4.07x |

The ratio is 2x to 4x and the absolute cost stays in single nanoseconds. Section
4.2 turns this into the step 4 acceptance gate.

### 11.3 The splice unit and the predicted keystroke tick

Root shape first, because it is why 5.1 changed:

| file | root trees | largest root entry covers | items in the entries block |
|---|---|---|---|
| aatrox.rito | 5 | 97.4% | 9 |
| azirultsoldier.rito | 5 | 99.7% | 2 |
| big.rito | 5 | 99.9% | 45 |
| zaahen.rito | **1** | 100.0% | 57 |
| skin38.rito | 5 | 100.0% | 144 |

Reparsing one item of the entries block, against reparsing the file. The tick adds
the tail shift from 11.1. The p95 item is the p95 by width.

| file | p95 item | whole file | tick | gain | shift share of the tick |
|---|---|---|---|---|---|
| azirultsoldier.rito | 146 us | 187 us | 148 us | 1.3x | 2% |
| aatrox.rito | 51.5 us | 510 us | 57.3 us | 8.9x | 10% |
| big.rito | 6.46 ms | 20.55 ms | 6.67 ms | 3.1x | 3% |
| zaahen.rito | 1.56 ms | 37.68 ms | 1.98 ms | 19.0x | 21% |
| skin38.rito | 0.93 ms | 70.27 ms | **2.30 ms** | **30.5x** | **60%** |

Three readings to carry forward. The splice is worth building, because it turns a
70 ms tick into a 2.3 ms tick on the largest file. The gain is not uniform, and
big.rito is the warning: its 45 items are so uneven that one of them is a third of
the file, so its tick only falls to 6.7 ms. The tick is proportional to the damaged
item and items are not all small, so this document should not promise a tick
proportional to the edit.

### 11.4 Parse throughput against working-set size

From the `whole_file` readings above.

| file | source | CST | parse | MiB/s |
|---|---|---|---|---|
| azirultsoldier.rito | 23 KB | 163 KB | 0.19 ms | 121 |
| aatrox.rito | 61 KB | 319 KB | 0.51 ms | 116 |
| big.rito | 1,325 KB | 9,090 KB | 20.55 ms | 63 |
| zaahen.rito | 2,231 KB | 16,160 KB | 37.68 ms | 58 |
| skin38.rito | 3,688 KB | 29,256 KB | 70.27 ms | 51 |

A 2.4x decline while the working set grows 180x. Section 6 reads this as support
for its premise, with the realloc confound named there.

### 11.5 Close-order against pre-order children

A full child walk over each layout, then the cost of reaching the pre-order layout
by a separate permutation pass.

| file | close order | pre order | gain | permutation | walks to break even |
|---|---|---|---|---|---|
| aatrox.rito | 5.22 us | 4.91 us | 1.06x | 11.6 us | 38 |
| azirultsoldier.rito | 2.50 us | 2.55 us | 0.98x | 5.8 us | never |
| big.rito | 191 us | 157 us | 1.22x | 339 us | 9.9 |
| zaahen.rito | 364 us | 299 us | 1.22x | 707 us | 11.0 |
| skin38.rito | 1.08 ms | 0.54 ms | **1.99x** | 1.79 ms | 3.3 |

### 11.6 Virtual wrapper nodes

A walk over an id stream where 46% of ids decode from the id, against a walk over
a full node array. The kept rows stay full rows, so this isolates the shorter array
and the bit-31 branch.

| file | all rows | virtual, compacted | ratio |
|---|---|---|---|
| azirultsoldier.rito | 1.25 us | 1.62 us | 0.77x |
| aatrox.rito | 2.48 us | 3.13 us | 0.79x |
| zaahen.rito | 179 us | 205 us | 0.87x |
| skin38.rito | 403 us | 436 us | 0.92x |
| big.rito | 100.0 us | 100.8 us | 0.99x |

Slower on every sample in the repository corpus. The larger sample in 11.8 reaches
1.013x, which is the only reading above parity. Section 6.4 carries the consequence.

### 11.7 What these readings changed

| claim | where | outcome |
|---|---|---|
| Tail shift is well under 1 ms per million tokens | 5.2 step 5 | **Wrong.** 2.46 ms per million tokens. The decision it supports survives. |
| An edit sits in one root entry, so search the root's children | 5.1, 5.2 step 1 | **Wrong.** One root entry is the whole file. Search the entries block one level down. |
| Item 3 is a win on memory and latency, and the cheapest of the four | 6.3 | **Half wrong.** The memory half stands. A permutation pass costs more than it saves. |
| Wrapper virtualization beats bit-packing | 6.4 | **Narrowed.** True for memory. It costs 1% to 23% of walk time. |
| `span(cst)` is two array reads | 4.2 | **Confirmed, and priced.** 2x to 4x a field read, about 2% of `build_bin`. |
| The pipeline is memory-bound | 6, 6.1 | **Supported.** Throughput falls 2.4x as the working set grows 180x. |
| The stored `children` array has a locality fault | 6.3 | **Confirmed.** A pre-order layout walks 1.99x faster on skin38. |

Two of these corrections point the same way. The tail shift is the larger half of
the splice tick, and the shift cost is set by the width of `Node` and `Child`. So
6.3's first two items, which were filed as memory savings that change no traversal,
are also the cheapest keystroke-latency work in this document.

### 11.8 A larger sample from outside the repository

`base_srx.materials.bin`, converted with
`cargo run --release --example bin_to_rito` and the CommunityDragon hash lists.
The file is not in the repository. To repeat the run, put the path in
`LTK_RITOBIN_BENCH_EXTRA`.

At 6,482 KiB of text it is 1.8x the largest sample in the corpus, and its CST is
45.9 MiB. It holds **691 entries**, against 144 in skin38, so its entries are much
smaller and it is the best case for a splice that this document has measured.

| reading | value | corpus comparison |
|---|---|---|
| source, CST | 6,482 KiB, 45.9 MiB | 1.8x skin38, 1.6x its CST |
| tokens, nodes, children | 913.6K, 828.2K, 1,741.8K | `children = tokens + nodes - 1` holds |
| parse | 122.9 ms, 51.5 MiB/s | matches skin38's 51 MiB/s |
| tail shift | 2.55 ms, 2.80 ms per 1M tokens | the highest in any sample |
| p95 item reparse | 575 us | 214x faster than the file |
| **splice tick** | **3.13 ms, a 39x gain** | the largest gain measured |
| **shift share of the tick** | **82%** | 60% on skin38 |
| span read, walk order | 0.76 ns against 2.61 ns, 3.46x | in line with skin38 |
| children, close against pre-order | 2.34 ms against 1.09 ms, **2.14x** | the largest gain measured |
| permutation to pre-order | 3.47 ms, 2.8 walks to break even | 3.3 walks on skin38 |
| wrapper nodes, virtual against all rows | 1.013x | the first sample above 1.00x |

Three readings sharpen the conclusions above.

**The splice and the shift move in opposite directions.** More entries make each
one smaller, so the reparse falls to 575 microseconds. The shift does not fall,
because it covers the whole file either way. On the sample best suited to
splicing, the shift is 82% of the tick. So the better the granularity gets, the
more completely `Node` and `Child` width sets keystroke latency. 6.3 items 1 and 2
are the work that matters here, and no reparse tuning substitutes for them.

**The children locality fault grows with the file.** 2.14x is the largest gain
measured, and the permutation break-even falls to 2.8 walks, which is about what an
`e2e` run does. The permutation is still not a clear win, but it is no longer
clearly a loss at this size. Native pre-order emission remains the version to
build.

**Wrapper virtualization reaches parity here, and no further.** 1.013x is the only
reading above 1.00x in seven samples. The rule stays as 6.4 states it. Small files
lose clearly, and large files are a wash. Nothing in the data supports taking the
item for speed.

---

## 12. Direction

Sections 1 to 11 decide many separate questions. This section states the one goal
they serve, ranks the work against it, and closes the questions that keep coming
back. Read it first and read the rest for the reasons.

### 12.1 The goal

**A keystroke must cost less than one frame, and should cost less than 5 ms, on
any ritobin file up to about 8 MB of text.** That covers every file this project
has seen. The LSP debounce exists today because the crate cannot meet it.

Where the two designs stand against that goal:

| | smallest sample | skin38, 3.6 MB | materials, 6.6 MB |
|---|---|---|---|
| full reparse, today | 0.19 ms | 70.3 ms | 122.9 ms |
| splice, projected (11.3, 11.8) | 0.15 ms | 2.30 ms | 3.13 ms |

The full reparse misses the goal above about 1 MB. The splice meets it on every
sample measured, with a 20x to 40x margin. **The goal is therefore not "make the
parser faster". It is "stop reparsing the file".** Every constant-factor item in
this document is a second-order effect on top of that one change.

### 12.2 The work, ranked by effect on the tick

Measured against the 3.13 ms tick of the largest sample (11.8).

| rank | work | effect on the tick | section |
|---|---|---|---|
| 1 | Stage 2, the entry-level splice | 122.9 ms to 3.13 ms, a 39x cut | 5.2 |
| 2 | Token-anchored node spans | a prerequisite for 1, and it costs about 2% of `build_bin` | 4 |
| 3 | Arena width: `Child` as a `u32`, the error side table, CSR children | the shift is 82% of the tick and scales with arena bytes. 48.1 MB falls to about 31 MB, so the tick falls about 28% | 6.3 items 1-3 |
| 4 | The parser-perf PR | it shortens the reparse third of the tick, so about 11%, and it gives 2.5x to 4x to every consumer that is not an editor | perf doc |
| 5 | `Newline` removal and token packing | benchmark-gated. Tokens are the smallest of the three arrays, so this ranks last for the tick | 6 items 1-3 |

Ranks 3 and 4 together take the largest sample from 3.13 ms to about 1.9 ms.
Neither is needed to reach the goal. Both are needed to keep reaching it as files
grow, which is 12.4.

**This changes two things in section 10.** The splice is currently filed under
"Deferred, not tracked yet" and gated on a measurement. Stage 0 and section 11
supply that measurement, so the splice is the tracked goal and not a maybe. And
section 10 ranks the array work by memory saved. Ranked by tick it is `Child` as a
`u32` first, then the error side table, then CSR.

### 12.3 The accessor decision is no longer on the critical path

9.1 says to settle the value-and-iterator surface before filing the section 9
issue, because reference-returning accessors freeze the layout. That still holds
for what it claims. What changed is the value of what it unlocks.

The three items that move the tick are all reachable with today's reference
accessors. `Child` as a `u32` and the error side table are already listed as
reachable in 9.1. CSR children join them, because a slice still comes back. Its
length just comes from the next node's start, so `ChildRange::get(cst)` becomes
`cst.children_of(id)`. That is a call-site change and not a surface change.

The two items that need the value surface are the derived child list (6.3 item 4)
and wrapper virtualization (6.4). Section 11 leaves the first unproven and shows
the second is not a speed win. **So 9.1 no longer blocks anything worth doing
soon.** Settle it on API-design grounds, in its own time, and stop treating it as
a gate on step 0.3.

### 12.4 The ceiling this design accepts

The tail shift is O(file) for every keystroke. That is inherent to a flat arena
with absolute offsets and dense ids, and section 3 accepts it deliberately. The
measurements now put a number on where it stops working. The shift is 2.55 ms at
6.6 MB, so a 30 MB file costs roughly 12 ms per keystroke before any reparse, and
the design stops meeting the goal near there.

Nothing in this document raises that ceiling, because rank 3 lowers the constant
and not the exponent. If the ceiling is ever reached, the answer is not relative
spans, which section 3 rejects on query cost. It is a per-segment base: cut the
token array into fixed-size segments, store a byte base per segment, and let a
splice update one segment plus the bases after it. A shift becomes O(segments), a
span query pays one extra load, and the segment index divides out of the token id.
The same trick applies to id renumbering. **This is a sketch and not a design. It
is not measured, and it is not needed at today's file sizes.** It is recorded so
the limit is a known boundary rather than a surprise.

### 12.5 Closed questions

Do not reopen these without new measurements. Each has a section holding the
reason.

| question | verdict | reason |
|---|---|---|
| Parent-relative or width-only spans | no | flat arena, so relocatability buys nothing and every query pays (3) |
| Offset and width instead of start and end | no | identical at `u32`, and narrower cannot hold an inverted span (3) |
| Gap-encoded tokens with checkpoints | no | removes 12% of the tick and charges a 64-entry scan on every span query forever (3, 11.1) |
| Hash-consing or shared green subtrees | no | needs position-independent subtrees, which token anchoring gives up on purpose (5.4) |
| Splicing at root-entry granularity | no | one root entry is 97% to 100% of the file (5.1, 11.3) |
| A permutation pass to reach pre-order children | no | costs 2.8 to 11 walks to break even, and a pipeline does 2 to 3 (6.3, 11.5) |
| Wrapper virtualization for speed | no | 0.77x to 1.01x across seven samples. Take it for memory or not at all (6.4, 11.6) |
| A memo cache on derived child lists | no | single-pass walks take every miss and no hit (6.3) |

### 12.6 Gates

Three checks keep the direction honest. Each names the group that answers it.

1. **Step 4 must hold `build_bin` flat.** `span(cst)` costs 2.2x to 4.1x a stored
   field read, which projects to about 2%. A regression above 5% means something
   else went wrong (4.2, 11.2).
2. **Rerun `entry_reparse/whole_file` after the parser-perf PR.** It separates the
   memory-bound premise from the realloc doublings that share its shape. A flat
   curve retires section 6 (6, 11.4).
3. **Bench the splice against the 5 ms goal on the largest file available, not the
   corpus.** The corpus understates the shift, because the shift grows with the
   file and the corpus tops out at 3.6 MB (11.8).
