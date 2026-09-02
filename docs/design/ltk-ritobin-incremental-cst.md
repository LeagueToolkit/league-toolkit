# ltk_ritobin incremental CST: API specification

Specifies the CST work for `ltk_ritobin`: the API each step lands, the order, and
the acceptance gate per step.

Rationale, rejected alternatives and benchmark readings live in
[`ltk-ritobin-span-model.md`](ltk-ritobin-span-model.md), cited here by section
number. [`ltk-ritobin-parser-perf.md`](ltk-ritobin-parser-perf.md) specifies the
constant-factor parser rework, which is an independent PR.

Scope: `cst/ids.rs`, `cst/tree.rs`, `cst/builder.rs`, `parse/parser.rs`, a new
`cst/splice.rs`, and the call sites in `typecheck/` and `print/`. Written
2026-08-28. Branch `fix/ritobin-node-error-ranges`.

---

## 1. Goal

Two budgets, because two jobs run at two different rates.

### 1.1 Syntax tick: under one frame, and under 5 ms, up to about 8 MB

Runs on every keystroke. It covers everything the CST alone answers: highlighting
by token kind, brace matching, folding ranges, indentation, selection expansion
and document symbols.

| | 23 KB | skin38, 3.6 MB | materials, 6.6 MB |
|---|---|---|---|
| full reparse, today | 0.19 ms | 70.3 ms | 122.9 ms |
| entry splice, target | 0.15 ms | 2.30 ms | 3.13 ms |

A full reparse misses this above about 1 MB. The splice meets it on every sample
with a 20x to 40x margin (span doc 11.3, 11.8). The objective is therefore to stop
reparsing the file, not to make the parser faster.

### 1.2 Semantic tick: under the debounce interval

`Cst::build_bin` produces the `Bin` and the type diagnostics, and it feeds hover,
completion and semantic highlighting. **It does not run per keystroke.** The
policy is a debounce on idle plus explicit triggers: a manual command, window
focus loss, and save.

`build_bin` runs at 120-180 MiB/s: 9 ms on big.rito, 30 ms on skin38 and about
55 ms at 6.6 MB. Every one of those fits inside a normal idle debounce, so **no
memo layer is required at today's file sizes**, and 8.6 stays deferred.

Two requirements follow. The semantic pass runs off the keystroke thread, and a
new edit cancels it, so a 55 ms pass never delays the next syntax tick.

### 1.3 Where the syntax tick goes

The splice tick has two parts:

| file | reparse one entry | tail shift | tick | shift share |
|---|---|---|---|---|
| skin38.rito | 0.93 ms | 1.38 ms | 2.30 ms | 60% |
| materials | 0.58 ms | 2.55 ms | 3.13 ms | 82% |

The shift is O(file) and the reparse is O(damaged entry), so the shift share rises
as entries get smaller. Arena width therefore sets keystroke latency, which places
step C in this plan rather than in a memory backlog.

---

## 2. Change set

| step | change | effect |
|---|---|---|
| 0 | Prerequisites: `push_errors` fix, propagation default, docs commit, two unused dependencies | correctness, and step D needs per-node errors |
| A | Fold `ChildRange` and `ErrorRange` into `IdxRange<T>`, add `TokenRange` | one definition for the shape step B needs a third copy of |
| B | Token-anchored node spans | prerequisite for step D, deletes the span bookkeeping |
| C | Arena width: `Child` as a `u32`, error side table, CSR children | 48.1 MB to about 31 MB, about 28% off the tick |
| D | Entry-level splice | the goal |

The parser-perf PR is a sixth item with its own document. It cuts about 11% of the
tick and gives 2.5x to 4x to every consumer that is not an editor.

Projected tick, assuming the shift scales with arena bytes:

| | arena, skin38 / materials | skin38 tick | materials tick |
|---|---|---|---|
| D alone | 29.3 / 48.1 MB | 2.30 ms | 3.13 ms |
| D + C1 + C2 | 21.0 / 34.5 MB | 1.92 ms | 2.41 ms |
| D + all of C | 19.0 / 31.2 MB | 1.83 ms | 2.23 ms |
| D + C + parser-perf | | ~1.2 ms | ~1.9 ms |

### 2.1 Order

Steps A to D are serial. Each rewrites call sites the next one touches.

| constraint | reason |
|---|---|
| `feat/ptch-resolve` lands before A | A migrates `get` call sites in the typecheck files that branch edits |
| #175 then #176 land between A and B | both are written against materialized spans, and B migrates them in one sweep |
| The option A spanned IR pairs with B | the IR meta becomes a `TokenId`. Open dependency: the meta A/B decision in the options doc |
| PR 185 rebases last | onto the world after B |
| parser-perf lands before B | B rebases its `build_tree` rewrite onto it. Independent of every other item |

---

## 3. API

`Span`, `Token`, `NodeId`, `TokenId` and `Kind` do not change.

```rust
// cst/ids.rs - one definition replaces ChildRange and ErrorRange, and adds TokenRange
pub struct IdxRange<T> {
    start: u32,
    len: u32,
    /// zero-sized; `fn() -> T` keeps the type `Copy + Send + Sync` for any `T`
    _marker: PhantomData<fn() -> T>,
}

pub type TokenRange = IdxRange<Token>;
pub type ChildRange = IdxRange<Child>;
pub type ErrorRange = IdxRange<Error>;

impl<T> IdxRange<T> {
    pub(crate) fn new(start: u32, len: u32) -> Self;
    pub fn empty() -> Self;
    pub fn empty_at(anchor: u32) -> Self;
    pub fn start(&self) -> u32;
    pub fn len(&self) -> u32;
    pub fn is_empty(&self) -> bool;
}

impl<T> Index<IdxRange<T>> for [T] { type Output = [T]; }
impl<T> IndexMut<IdxRange<T>> for [T] {}

/// The empty-range anchor contract belongs to the token instantiation alone.
impl IdxRange<Token> {
    pub fn first(&self) -> Option<TokenId>;
    pub fn last(&self) -> Option<TokenId>;
    pub fn ids(&self) -> impl Iterator<Item = TokenId>;
}
```

```rust
// cst/tree.rs
pub struct Node {
    pub kind: Kind,
    pub tokens: TokenRange,     // step B: replaces `span: Span`
    pub children: ChildRange,
}                               // step C2: `errors` moves to the side table

impl Node {
    pub fn span(&self, cst: &Cst) -> Span;              // step B
    pub fn open_brace_span(&self, cst: &Cst) -> Span;   // unchanged
}

/// A packed child. Bit 31 selects the array. Step C1.
#[repr(transparent)]
pub struct Child(u32);

pub enum ChildKind { Token(TokenId), Tree(NodeId) }

impl Child {
    pub fn token(id: TokenId) -> Self;
    pub fn tree(id: NodeId) -> Self;
    pub fn kind(self) -> ChildKind;
    pub fn is_token(self) -> bool;
    pub fn is_tree(self) -> bool;
    pub fn span(self, cst: &Cst) -> Span;
}

impl Cst {
    pub fn nodes(&self) -> &[Node];
    pub fn children(&self) -> &[Child];
    pub fn tokens(&self) -> &[Token];

    /// Every error in the tree, under every propagation mode.
    pub fn errors(&self) -> &[Error];

    pub fn children_of(&self, id: NodeId) -> &[Child];
    pub fn errors_of(&self, id: NodeId) -> &[Error];

    pub fn revision(&self) -> Revision;
    pub fn verify(&self) -> Result<(), VerifyError>;
}

/// Identifies one state of one tree. Drawn from a process-wide counter, never
/// from zero, so two trees cannot share a value. `Copy + Eq + Hash`.
///
/// Answers "did this tree change at all". Validates no handle: see 10.3.
pub struct Revision(u64);
```

```rust
// cst/splice.rs - step D
pub struct Edit {
    /// The replaced range, in the text this `Cst` was parsed from.
    pub range: Span,
    /// The byte length of the replacement text.
    pub new_len: u32,
}

pub enum SpliceRejected {
    /// The tree was not built with `ErrorPropagation::None`.
    Propagation,
    /// The file has no top-level `entries` block, or the edit is outside it.
    OutsideEntries,
    /// The edit touches more than one item, or the block's own tokens.
    NotOneItem,
    /// The relexed token stream does not realign with the old one.
    Resync,
}

impl Cst {
    /// Splices `edit` in place. On `Err` the tree is unchanged.
    pub fn splice(&mut self, new_text: &str, edit: Edit) -> Result<(), SpliceRejected>;

    /// `splice`, falling back to a full reparse. The editor entry point.
    pub fn apply_edit(&mut self, new_text: &str, edit: Edit) -> EditOutcome;
}

pub enum EditOutcome { Spliced, Reparsed(SpliceRejected) }
```

```rust
// parse/tokenizer.rs - step D

/// The entire cross-token state of the lexer.
///
/// Relexing from a token boundary needs this and nothing else. Any lexer change
/// that adds cross-token state must add a field here, or the splice loses the
/// resync guarantee in 8.3 step 2. The fields are private, so a new one is not a
/// breaking change (C-STRUCT-PRIVATE).
///
/// `Default` is the state at the start of a file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LexState {
    /// Whether the previous token ended a value. `ends_line` reads this to decide
    /// synthetic `Newline` emission.
    ends_value: bool,
}

impl LexState {
    /// The state after `token`.
    pub fn after(self, token: &Token) -> Self;
}

/// Lexes from byte offset `at`, which must be a token boundary, in the given state.
pub fn lex_from(source: &str, at: u32, state: LexState) -> impl Iterator<Item = Token> + '_;

/// Unchanged, and now defined as `lex_from(source, 0, LexState::default()).collect()`.
pub fn lex(source: &str) -> Vec<Token>;
```

Naming: `Child::kind` follows the crate vocabulary (`Node.kind`, `Token.kind`) and
C-GETTER, which spells a getter `field()` and never `get_field()`. `children_of`
and `errors_of` carry the suffix because `Cst::children` is already the whole
array.

---

## 4. Step 0: prerequisites

Four independent items, all available now.

**0.1 The `push_errors` fix.** `Cst::push_errors` computes both ends of the range
from `children.len()` instead of `errors.len()`, so every node-attached
`ErrorRange` is `len: 0` with a start from the wrong vector. Consumers read the
flat `cst.errors` vector, which hides it. Must precede step A, which deletes the
`ErrorRange::get` clamp that masks it. Done in `d293dfa` on
`fix/ritobin-node-error-ranges` with a regression test. The PR to `main` is
outstanding.

**0.2 `ErrorPropagation::None` becomes the default.** Step D requires it. `Move`
drains every child error into the parent, so the root owns the whole list and a
damaged entry holds no handle on the errors it raised. Under `None` an error
splices out with its subtree like the other three arrays.

Four statements disagree today: `#[default]` sits on `None`, the doc comment on
`Move` claims to be the default, `Cst::parse` passes `Move`, and the `lib.rs`
crate doc repeats the `Move` claim. After this item `Cst::parse` passes `None` and
the comments follow the code. `Cst.errors` keeps its meaning as the complete flat
list under every mode. This breaks any caller that reads `cst.root().errors`, and
the flat list replaces it. `Cst` records the mode it was built with, so step D can
reject a `Move` tree.

**0.3 Commit the design docs, file the tracking issue.** The `docs/design/*.md`
files are untracked and the issues need permalinks. The issue body is section 3
plus the migration lists. Mark it breaking.

**0.4 Settle two unused dependencies.** `ltk_ritobin/Cargo.toml` lists `salsa`
0.22 and `xxhash-rust`. Neither appears in `src/`, `benches/`, `tests/` or
`examples/`. `xxhash-rust` gains a use in 10.4, so it stays. `salsa` is a large
tree carried for nothing: record the intent next to it or drop it until the memo
layer of 8.6 exists.

---

## 5. Step A: the `IdxRange<T>` fold

`refactor(ritobin)!: fold the CST ranges into one generic`

`ChildRange` and `ErrorRange` are the same eight bytes with the same two methods,
and step B needs a third copy. One marker-generic struct replaces all three.
la-arena's `IdxRange<T>` in rust-analyzer is this design.

The marker keeps `cst.errors()[node.children]` a type error, which one untyped
range would not. A trait cannot reach fields, so each alias would still hand-write
its accessors and every call site would need the trait in scope. That is the
friction the value API review filed against `PropertyExt` (Q5,
M-ESSENTIAL-FN-INHERENT).

Four details:

1. **Out of bounds panics, uniformly.** The generic `Index` panics like a slice.
   Today's `ErrorRange::get` clamp is a band-aid for 0.1, and it is partial anyway.
   A range straddling the end still panics, and one fully past the end vanishes.
2. **Serde:** `#[serde(skip)]` on the marker plus `#[serde(bound = "")]`. The wire
   shape `{start, len}` is unchanged.
3. **Debug is hand-written** as `start..start+len`, so snapshots grow no
   `PhantomData` noise.
4. **Arithmetic widens before it adds.** The fields stay `u32`, because element
   size is bandwidth in every hot pass. `start + len` computes as
   `start as usize + len as usize`, never in `u32`. The fields are private, so
   only the `Index` impl and `Node::span` do this arithmetic.

**Migration.** `range.get(cst)` becomes `cst.children_of(id)` where the caller
holds the id, which is 12 of the 16 sites, and `&cst.children()[range]` in the
other 4. Both hand-written `get` bodies and the clamp are deleted. `Cst.errors`
becomes private behind `errors()`. `children_of` arrives here rather than later
because it makes the C3 change internal.

**Acceptance.** No behavior change, no snapshot churn, `cargo clippy
--all-targets` clean.

---

## 6. Step B: token-anchored node spans

`feat(ritobin)!: anchor node spans to the token array`

Nodes stop storing byte spans and store their token range. The token vector
becomes the single source of truth for position.

### 6.1 Layout and resolution

```rust
pub struct Node {
    pub kind: Kind,
    pub tokens: TokenRange,   // replaces `span: Span`, same 8 bytes
    pub children: ChildRange,
    pub errors: ErrorRange,   // step C2 removes this
}

impl Node {
    pub fn span(&self, cst: &Cst) -> Span {
        match self.tokens.len() {
            0 => {
                let at = cst.tokens().get(self.tokens.start() as usize)
                    .map_or(cst.source_len, |t| t.span.start);
                Span::new(at, at)
            }
            n => {
                let start = self.tokens.start() as usize;
                let first = &cst.tokens()[start];
                let last = &cst.tokens()[start + n as usize - 1];
                Span::new(first.span.start, last.span.end)
            }
        }
    }
}
```

Two array reads, no walk. The node keeps its size and carries more: byte positions
stay available, and the token addressing step D needs comes free.

**Empty nodes.** A node that consumes no tokens stores `len: 0` with `start` set
to the index of the token that would come next. Its span resolves to an empty span
at that token's start, or at end of source when the anchor is one past the last
token. This replaces today's convention, an empty span at the previous token's
end, with one that cannot go stale, and it handles EOF without a special case.

### 6.2 Deletions in `build_tree`

- the `start_known` flag on `StackItem` and its propagation at `Close`
- the parent-span widening, a `min` and a `max` on every `Close`, through a
  `split_at_mut`
- the `last_span` threading and the `span.end == 0` fixup for empty trees

The builder records the token index at `Open` and the count at `Close`: two
integer assignments, no branch per token. A node span cannot disagree with its
tokens, because it no longer exists as stored data.

### 6.3 Span values change

`Node.span` does not hold the token-derived span today. `build_tree` seeds a node
at `Open` with `Span::new(last_span.end, 0)` and the `Close` path only applies
`min`, so the seed is a floor that nothing raises. A node whose first child is a
subtree keeps the seed and absorbs the trivia in front of its own first token.

On a small realistic file, 11 of the 55 nodes that own tokens disagree with their
tokens. The affected kinds are `Entry` and `EntryValue`.

```text
EntryValue: stored  25..32  " \"PROP\""   derived  26..32  "\"PROP\""
Entry:      stored 105..181 "\n    \"a\" = B {...}\n"
            derived 110..181 "\"a\" = B {...}"
```

`typecheck/walk.rs` passes the node span to `default_span`, and
`typecheck/resolve.rs` stores it as IR meta, so diagnostics move by a few
characters and the `insta` snapshots churn. That churn is the expected result of
this step. This step must not inherit the bit-identical-output invariant of the
parser-perf PR.

### 6.4 The `CstBuilder` token order fix

`Node::span(cst)` requires tokens in source order. `CstBuilder` breaks that in
four places:

- `block` creates both braces before the children, which already hold lower ids
- `entry_tree` creates the separators before the key, the type and the value
- the string arm uses one `Quote` id at two positions in the tree
- `entry_tree` allocates a `Colon` that it drops when there is no type

A dump shows token ids walking `[0, 2, 4, 3, 5, 1, 7, ...]`, one `Quote` id twice,
and one `Colon` in no node. The fix allocates ids in emission order: create each
token at the position it occupies. Builder node spans are already meaningless,
because most builder tokens carry `Span::default()`, so this step makes a quiet
fault load-bearing and the fix needs its own test.

### 6.5 Riders

Three items belong here because this step rewrites the same code.

| rider | content |
|---|---|
| `Cst::revision` | One accessor over one `u64`. Constant until step D. Answers "did this tree change at all" and nothing more (10.3) |
| `VisitCtx::text_of(id)` | The #175 note. The one call every consumer of `span(cst)` wants |
| `Cst::verify` | Asserts the three-array invariant that step D depends on (8.2) |

### 6.6 Acceptance

1. The `build_bin` bench group stays flat. `span(cst)` costs 2.2x to 3.6x a stored
   field read in walk order and `build_bin` reads a span per node, which projects
   to about 2% (span doc 4.2, 11.2). A regression above 5% means the migration
   went wrong elsewhere.
2. Snapshot churn is reviewed span by span against 6.3, never accepted in bulk.
3. The `CstBuilder` fix has a test asserting token ids ascend in a built tree.
4. `Cst::verify` passes on every corpus sample.

---

## 7. Step C: arena width

`perf(ritobin)!: pack the child array and move errors to a side table`

Three items. C1 and C2 are unconditional and change no traversal. C3 is
conditional on a measurement taken during step B.

Structural facts, true on every corpus sample: `children` is exactly
`tokens + nodes - 1`, every id needs 20 bits at most so bit 31 is free, and tree
depth reaches 47.

**C1. Pack `Child` into a `u32`.** The enum is 8 bytes carrying 33 bits. The tag
goes in bit 31, with an assertion that the id fits. `children` halves. This forces
the accessor in section 3: about 20 sites change from
`match child { Child::Token(t) => .. }` to
`match child.kind() { ChildKind::Token(t) => .. }`.

**C2. Move `ErrorRange` off `Node` into a side table.** Nodes carrying errors are
0 of 5,580, 1 of 156,938 and 0 of 501,572 across the corpus, so the field wastes 8
bytes on every other node. A sorted `Vec<(NodeId, ErrorRange)>` with a binary
search puts the cost on a path almost nothing takes, and `errors_of` stays a slice
accessor. `Node` falls from 28 bytes to 20. C1 and C2 together take the largest
sample from 48.1 MB to 34.5 MB.

**C3. Emit `children` in pre-order and drop the length.** Conditional: **take C3
only if the pre-pass below costs less than the 535 microseconds per walk it saves
on skin38. Otherwise stop at C1 and C2 and leave the arena at 34.5 MB.**

`build_tree` appends a node's slice at `Close`, which is post-order, so
`ChildRange.start` is not monotone in `NodeId`. A pre-order walk reads `nodes`
with a mean absolute jump of 1.0 entries and `children` with a mean jump of 42.9
entries, 54% of them backward. A pre-order child array walks 1.99x faster on
skin38 and 2.14x faster on materials. In pre-order `len` becomes
`start[i + 1] - start[i]`, which is CSR adjacency layout: `ChildRange` halves to 4
bytes, `Node` reaches 16, and the arena falls to 31.2 MB.

A separate permutation pass reaches that layout but loses. It costs 1.79 ms on
skin38 and 3.47 ms on materials, needing 2.8 to 11 walks to break even, and a
pipeline does 2 to 3. Only native pre-order emission wins, which requires
`build_tree` to know each node's child count before writing the slice. One route
is a pre-pass over the event stream counting `Advance` and `Close` per open node
in pre-order, prefix-summed into starts. It costs one extra scan of an event array
the parser-perf PR already shrinks to about 3 MB. Price that pre-pass during step
B, which rewrites this loop anyway.

**Acceptance.** Bench `parse` and `build_bin` at each item. C1 and C2 hold both
flat. C3 shows the walk gain on `build_bin` and loses none of it back in `parse`.

---

## 8. Step D: the entry-level splice

`feat(ritobin): splice a single entry instead of reparsing the file`

The goal. Needs its own tracking issue, sequenced after step B.

### 8.1 Format properties the splice uses

**The splice unit is one item of the top-level `entries` block.** A file has 1 to
5 root entries, because `entries: map[hash, embed] = { ... }` holds the whole
payload. The largest root entry covers 97.4% to 100.0% of the file, so a
root-level splice gains nothing. The block holds 2 to 144 items in the corpus and
691 in materials. Reaching it is three fixed hops, `File` to `Entry` to
`EntryValue` to `Block`, then one search over that block's children.

**No content token crosses a line break.** Strings become `UnterminatedString` at
a newline and comments end at the newline. Only the synthetic `Newline` token
spans line breaks, and it regenerates from the whitespace run.

**The lexer carries one bit of cross-token state.** `ends_line` is the only place
in `lex` that reads anything outside the run it is scanning, and it reads one
predicate over the previous token: `ends_value(kind)`. `Cursor` is a byte position
and nothing else. Relexing therefore restarts at any token boundary given that one
bit, which section 3 names `LexState`.

This is a property of the lexer, not of the ritobin format, so it is an invariant
to maintain rather than a fact to rely on. String interpolation or nested comments
would each add state. `LexState` exists so that such a change has to be declared,
and 8.5 gives it a test.

### 8.2 The three-array invariant

A subtree occupies one contiguous run of `nodes`, one of `children` and one of
`tokens`. `nodes` gets ids at `Open`, which is pre-order, so the run starts with
the subtree root. `children` flushes at `Close`, which is post-order, so the run
ends with the root's own slice. `tokens` is lexer order and has no anchor. That
difference drives 8.3 steps 5 and 6.

A check over every node of a sample file confirms all three runs. `Cst::verify`
asserts it, because steps 4 to 6 are a splice only while it holds. C3 moves
`children` to pre-order, which keeps the run contiguous and moves the root's slice
to its front.

### 8.3 The algorithm

For an edit replacing `[a, b)` with `n` bytes, `delta = n - (b - a)`:

1. **Locate.** Descend to the entries `Block`. Binary search its tree children by
   token range for the items touching `[a, b)`. Reject unless exactly one item
   contains the edit and the edit touches none of the block's own tokens.
2. **Relex.** Call `lex_from` at the item's first token boundary in the new text,
   with the `LexState` carried from the token before it. Stop at the first
   produced token that realigns with the old stream past the edit. **Realignment
   requires both the token boundary and the `LexState` to match**, never the
   boundary alone. A boundary-only check is sound today because `LexState` is one
   bit derived from the token kind, and unsound the moment the lexer gains state
   the check does not compare. Reject if the realignment point is past the end of
   the item.
3. **Reparse** the item with the existing `impls`, building the event stream into
   replacement ranges. Reject if the subtree comes back unbalanced.
4. **Splice** the four vectors, replacing the item's runs.
5. **Fix the ancestor path.** Ancestors sit on both sides of the splice: their
   rows are in the prefix of `nodes` because pre-order gives them low ids, and
   their child slices are in the tail of `children` because close order flushes a
   parent after every descendant. Each ancestor takes
   `tokens.len += delta_tokens`, `children.start += delta_children` and
   `errors.start += delta_errors`. Their `tokens.start` does not move and their
   child count does not change, because one item is one `Child::Tree` entry either
   way. The pass runs at most one row per level of depth.
6. **Shift the tails.** Three passes of two shapes.

   | pass | fields | shape |
   |---|---|---|
   | `nodes`, rows past the item's run | `tokens.start`, `children.start`, `errors.start`, by their element deltas | unconditional: each field is monotone in node id over that suffix |
   | `tokens`, past the item's run | `span.start`, `span.end`, by `delta` bytes | unconditional |
   | `children` and `errors`, past the item's runs | the packed id by `delta_tokens` or `delta_nodes`, the error span by `delta` bytes | **conditional** |

   An unconditional add over `children` or `errors` corrupts the tree. A child
   entry in the tail can reference an id in the prefix, because the ancestors'
   slices flush last and reference the items before the damaged one. An error in
   the tail can carry a span before the edit, because an ancestor's own error is
   pushed at its `Close`. Both passes compare before they add, on the referenced
   id and on the span start. The reverse case cannot happen: a prefix entry never
   references an id past the cut, because a node with a higher pre-order id is
   either inside the item or entirely after it. Compare-and-add vectorizes as a
   masked add, so the cost is what span doc 11.1 measures.
7. **Take a new `Revision`** from the process-wide counter (10.3).

A worked edit adding 14 bytes, 6 tokens and 3 nodes to the middle item of a
three-item block. Ranges are half-open.

| array | prefix | item, before | item, after | tail, before | tail, after | delta |
|---|---|---|---|---|---|---|
| `tokens` | [0, 40) | [40, 70) | [40, 76) | [70, 120) | [76, 126) | +6 |
| `nodes` | [0, 30) | [30, 52) | [30, 55) | [52, 90) | [55, 93) | +3 |
| `children` | [0, 60) | [60, 111) | [60, 120) | [111, 209) | [120, 218) | +9 |

The child delta is not free: `children = tokens + nodes - 1` holds on every
sample, so `delta_children = delta_tokens + delta_nodes` exactly. The errors array
is omitted because a whole file carries a handful, so its deltas are usually 0 and
often negative.

Step 6 exists in any flat-arena splice. Token anchoring is what confines the
byte-offset shift to the token array and the stored error spans, leaving every
node position correct without a fix.

C3 shortens step 5: in pre-order an ancestor's slice precedes its descendants', so
its `children.start` moves into the prefix and needs no fixup. This document does
not price that.

### 8.4 Rejection and fallback

`splice` returns `Err` and leaves the tree untouched. `apply_edit` reparses. Any
edit that rebalances braces, spans two items or lands on the root entries takes
the fallback, which is correct behavior and not a failure to handle.

### 8.5 Acceptance

1. **Equivalence, as a differential test.** For each corpus file, apply random
   edits and assert that a spliced `Cst` equals
   `Cst::parse_with_config(new_text, None)` field by field across `nodes`,
   `children`, `tokens` and `errors`. A rejection is a pass. Exact equality is
   reachable because reparsing one item produces what a full parse produces, and
   `None` keeps error ownership local.
2. A fuzz target over the same property, seeded with the corpus.
3. **`LexState` is complete, as a property test.** For every token boundary `b` in
   a corpus file, `lex_from(source, b, state_at(b))` yields the same token suffix
   as `lex(source)` from `b`. This is the invariant 8.1 names, and it is what
   catches a later lexer change that adds state without adding a field. It also
   pins `lex(source) == lex_from(source, 0, LexState::default()).collect()`.
4. The tick meets section 1 on the largest file available, not on the corpus
   (gate 3 in section 9).
5. `Cst::verify` passes after every splice in the differential test.

### 8.6 What the splice does not solve

The splice serves the syntax tick only. `build_bin` costs 9 ms to 30 ms on the
corpus and about 55 ms at 6.6 MB, which the debounce policy of 1.2 absorbs
without any incremental work. No memo layer is needed until `build_bin` exceeds
the debounce interval, which happens near 30 MB of text.

When it is needed, the typechecker extends the same way the parser does: entries
are independent apart from the four root entries and shadow detection, so it can
recheck damaged entries and patch the `Bin`. The natural key is a per-entry
content digest (10.4). Out of scope here, recorded so the span model is not the
blocker.

Arena handles do not survive the splice. `NodeId`, `TokenId` and `TokenRange` are
indices and step 6 renumbers all three, which is what makes them 4 bytes and the
shift a memcpy-class pass. Sections 10.3 and 10.4 hold the consequences.

---

## 9. Benchmarks and gates

```bash
cargo bench -p ltk_ritobin --bench incremental
cargo bench -p ltk_ritobin --bench parse
LTK_RITOBIN_BENCH_EXTRA="C:/path/to/big.rito" cargo bench -p ltk_ritobin --bench incremental
```

The corpus is `crates/ltk_ritobin/samples/`, which tops out at 3.6 MB.
`LTK_RITOBIN_BENCH_EXTRA` takes a semicolon-separated list of extra files.

| group in `benches/incremental.rs` | answers | gates |
|---|---|---|
| `splice_tail_shift` | cost of 8.3 step 6 across all three arrays | C |
| `node_span` | stored against derived span reads, walk and random order | B |
| `entry_reparse` | one entries item against the whole file | D |
| `children_layout` | close order against pre-order, plus the permutation cost | C3 |
| `wrapper_nodes` | a compacted node array against the full one | 10.2, out of scope |

Three gates:

1. **Step B holds `build_bin` flat.** About 2% projected, 5% is the line (6.6).
2. **Rerun `entry_reparse/whole_file` after the parser-perf PR.** Parse throughput
   falls from 121 MiB/s to 51 MiB/s while the working set grows 180x, which
   supports the memory-bound premise under step C and the token packing. The `Vec`
   doubling reallocs that the perf PR removes are a confound with the same shape.
   A curve that stays sloped confirms the premise. A flat curve retires the token
   packing.
3. **Bench the splice against the 5 ms goal on the largest file available.** The
   corpus understates the shift, which grows with the file.

---

## 10. Deferred and out of scope

### 10.1 Closed questions

Reasons live in the span model document, cited by section. Do not reopen without
new measurements.

| question | verdict | reason |
|---|---|---|
| Parent-relative or width-only spans | no | flat arena, so relocatability buys nothing and every query pays (3) |
| Offset and width instead of start and end | no | identical at `u32`, and narrower cannot hold an inverted span (3) |
| Gap-encoded tokens with checkpoints | no | removes 12% of the tick and charges a 64-entry scan on every span query forever (3, 11.1) |
| Hash-consing or shared green subtrees | no | needs position-independent subtrees, which token anchoring gives up on purpose (5.4) |
| Splicing at root-entry granularity | no | one root entry is 97% to 100% of the file (5.1, 11.3) |
| A permutation pass to reach pre-order children | no | 2.8 to 11 walks to break even, and a pipeline does 2 to 3 (6.3, 11.5) |
| A memo cache on derived child lists | no | single-pass walks take every miss and no hit (6.3) |

### 10.2 Measured, not worth building yet

**Wrapper virtualization** (span doc 6.4). 71.6% of nodes hold one child.
Encoding the 46% that hold one token into the `NodeId` saves about 12.7 MB on
skin38. It measures 0.77x to 1.013x across seven samples. Take it if memory
becomes the binding constraint, never for speed.

**Derived child array** (span doc 6.3 item 4). Removes 8.3 MB, taking skin38 to
2.65x the source. Unproven, and it needs the value-and-iterator surface.

**`Newline` removal and token packing** (span doc 6 items 1-3). Removing
`Newline` cuts 10.5% to 15.8% of tokens, events and children at once. Packing
takes the skin38 token array from 6,562 KB to about 3,400 KB. Tokens are the
smallest of the three arrays, so this ranks last, and gate 2 decides whether it
has a rationale at all.

### 10.3 Handle validation across an edit

**Ships: `Cst::revision()` and nothing else.** One accessor over one `u64`, drawn
from a process-wide `AtomicU64` rather than from zero per tree, so two files open
at once cannot collide. It answers "did this tree change at all", which invalidates
a memo table wholesale. It validates no handle.

A stale `NodeId` indexes in bounds and returns the wrong node with no panic.
`NodeId` cannot defend itself: carrying a revision makes it 8 bytes, which kills
C1, doubles the children array and lengthens the shift. Any check needs a wider
opt-in handle type, and three facts rule that out for now.

1. **Dominated by the fallback.** 8.4 makes rejection normal, and a full reparse
   produces a fresh tree with a fresh revision and no rebase record. Every anchor
   dies on that path.
2. **Opt-in where the mistake is not.** `Cst::node(&self, id: NodeId)` is public
   and `NodeId` is `Copy + Eq + Hash + Serialize`, so nothing stops a consumer
   storing a raw id across an edit.
3. **No consumer to price it against.** `build_bin`, the printer and the
   typechecker each walk once inside a single borrow and hold nothing.

Two mechanisms beat an opt-in anchor when a consumer exists.

**Lifetime-branded ids.** `NodeId<'a>` carries `PhantomData<&'a Cst>` and is
minted from `&'a self`, so `splice(&mut self)` will not compile while an id is
live. Zero bytes, zero branches, enforced rather than requested. Price it against
a second raw id type for `Child`, and against lifetime infection through
`Visitor` and the map-key uses.

**Debug-only wide ids.** `NodeId` stays a `u32` in release and carries the
revision under `cfg(debug_assertions)`, checked inside `Cst::node`. It protects
every consumer, costs nothing in release, and fires in CI. Price it against a
public type whose layout depends on the build profile.

Not the answer: a generation counter in the 11 spare bits of `NodeId`. A
generation answers "was this slot reused", and the failure here is that the whole
suffix renumbers at once.

If a rebase is ever wanted, the splice already computes `cut_start`, `cut_end_old`
and `delta_nodes`. Keeping those per revision turns a lookup into a rebase: below
`cut_start` unchanged, inside the cut gone, at or past `cut_end_old`
`id + delta_nodes`.

### 10.4 `NodeKey`, the durable identity

**Ships: the constraint, not the code.** No consumer holds anything across an edit,
so building this now repeats the mistake of 10.3. What is timely is the constraint
on #176, which lands between steps A and B and defines per-revision vocabulary:
**#176 must not adopt `ltk_meta::path::PropertyPath` as the durable identity.**

`PropertyPath` is `pub struct PropertyPath(String)`, a wire string in Riot's
grammar (`Position.UIRect.Size`, `Elements[3]`, `PerAttachmentMaterial{"weapon"}`).
Four faults, in increasing weight:

1. A string in a foreign grammar, capped by the PTCH wire format's `u16 pathLen`.
2. Not a whole address: a record addresses `(objectHash, PropertyPath)`.
3. Resolving it needs a valid tree, because segments carry no kind.
4. **Partial over CST nodes.** It names a property. It cannot name a comment, a
   `type:` or `version:` root entry, the `entries` block, an `EntryTerminator`, an
   `ErrorTree`, a token, a shadowed duplicate, or anything inside a malformed
   region. An editor anchors to whatever the cursor is in.

A durable key must be **total** over CST nodes, for the same reason the CST is
resilient.

```rust
/// Names a node across edits. Total over the CST, ritobin's own.
pub struct NodeKey {
    /// The entries-block item holding the node, or `None` above it.
    entry: Option<EntryKey>,
    /// One segment per level below that anchor.
    tail: SmallVec<[KeySeg; 6]>,
}

/// The CST keeps shadowed duplicates, so the occurrence index is part of identity.
pub struct EntryKey { hash: BinHash, occurrence: u32 }

pub enum KeySeg {
    /// A property, by FNV-1a name hash. Survives insertion of siblings.
    Field(u32),
    /// A container item or any unnamed child. Total, but positional.
    Index(u32),
}
```

The stable prefix is exactly the splice unit, so an edit inside one entry can
shift the positional tail of keys inside that entry and nothing else. That is the
blast radius the splice already has, on the entry a consumer re-walks anyway.

Rejected for the prefix: a pure positional ordinal, rust-analyzer's
`ErasedFileAstId`. It is total and fits a `u32`, but inserting one entry at the top
shifts every following ordinal. It survives as the `Index` fallback.

**Hashing, three separate questions.**

**Fingerprint the key**, for map storage:
`NodeKey::fingerprint() -> KeyHash(u64)` via `xxhash_rust::xxh3::xxh3_64`. xxh3
is deterministic across processes and platforms. `DefaultHasher` is randomly
seeded per process and would silently stop matching after a restart. xxh3 is also
faster than xxh64 on short inputs. `Xxh3` implements `std::hash::Hasher`, so
`NodeKey` derives `Hash` and needs no byte encoding. At 10^5 nodes and 64 bits the
birthday probability is about 3e-10 per file.

**Build one map per revision.** A single walk fills
`HashMap<KeyHash, NodeId>`, so a lookup costs O(1) instead of an O(depth) descent.
The map composes with the splice: rebuild it for the damaged entry and shift the
rest by the node delta. This is the `AstIdMap` of rust-analyzer. Give it a
passthrough `BuildHasher`, because the key is already a hash.

**Detect content change with `xxh3_128`** over the subtree source, which a
`TokenRange` makes one contiguous slice. It enables memo cutoff and cut-paste move
detection. It is never identity: two identical entries hash alike, which is
correct for cutoff and wrong for anchoring. The key must include the environment,
because the meaning of an entry depends on the root `type:` and `version:` entries
and on shadowing.

`xxhash-rust` is a workspace dependency with the `xxh3` feature on and
`ltk_ritobin` already lists it, so this adds nothing (0.4). `BinHash` stays
fnv1a-32 inside `EntryKey` and `WadHash` stays xxh64, because those match hashes
Riot defines. xxh3 covers the hashes ritobin invents.

PTCH interop is a conversion, never a dependency:

```rust
impl NodeKey {
    pub fn to_property_path(&self, cst: &Cst) -> Option<(BinHash, PropertyPath)>;
    pub fn from_property_path(obj: BinHash, p: &PropertyPath, cst: &Cst) -> Option<Self>;
}
```

Partial in both directions on purpose, so `NodeKey` stays definable and testable
without `PropertyPath` in scope.

Naming: #176 lands `NodePath` as a per-revision positional path. `NodeKey` is the
durable one, and the two must not be conflated.

### 10.5 Decisions with no deadline

**The value-and-iterator accessor surface** (span doc 9.1). Reference accessors
freeze the layout. Values with iterators make every item in 10.2 reachable for one
mechanical migration. All three items of step C work with today's reference
accessors, C3 included, so this gates nothing here. Deciding early still costs one
migration instead of two. Decide it on API-design grounds.

**The O(file) ceiling** (span doc 12.4). The tail shift is 2.55 ms at 6.6 MB, so a
30 MB file costs roughly 12 ms per keystroke before any reparse. Step C lowers the
constant, not the exponent. The shape of an answer is a per-segment byte base,
which makes a shift O(segments) and charges a span query one extra load. That is a
sketch. It is not measured, and it is not needed at today's file sizes.

Both ceilings land in the same place. `build_bin` at 120-180 MiB/s outgrows a
300 ms debounce near 39 MB, and the tail shift outgrows the frame budget near
30 MB. So one file size, near 30 MB, retires both the splice and the debounce
policy at once. Neither has a consumer today, and the two should be reconsidered
together rather than separately.
