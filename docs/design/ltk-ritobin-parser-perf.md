# ltk_ritobin parser perf: shrinking the event pipeline

Standalone constant-factor rework of `ltk_ritobin::parse`. Split out of the span
model doc (its section 6.1 holds the profile and rationale); this doc is the API
proposal and landing plan. Written 2026-08-27.

It is one item of the train in
[`ltk-ritobin-incremental-cst.md`](ltk-ritobin-incremental-cst.md), which holds
the CST plan this lands into. It must land before that document's step B, and it
answers that document's gate 2.

## 1. Scope and independence

Goal: cut `Cst::parse` time ~2.5-4x by removing memory traffic from the event
pipeline. No changes to `Cst`, `Node`, `Span`, spans-on-nodes, `Child`, ids, or
any downstream consumer. The produced `Cst` is byte-identical to today's
(section 5), so no snapshot churn.

Independence, verified against the branches in flight:

- `feat/ptch-resolve` touches `typecheck/*` and `types.rs`, nothing under
  `src/parse/` - no conflict.
- PR 185 touches `src/parse/span.rs` by one line - trivial.
- The span-model train (span doc section 10): this PR lands before step 4;
  step 4's `build_tree` rewrite then rebases onto the new builder and deletes
  the span bookkeeping this PR deliberately leaves alone.
- Hard prerequisite: the `push_errors` bugfix PR (span doc section 10, step
  0.1) lands first, with its regression test, so this rewrite preserves fixed
  semantics rather than buggy ones.

Baseline (release, dev machine, 2026-08-27):

| file | size | lex | event-gen | build_tree |
|---|---|---|---|---|
| big.rito | 1.29 MB | 1.2 ms | 8.5 ms | 9.3 ms |
| skin38.rito | 3.60 MB | 3.6 ms | 31.1 ms | 31.4 ms |

Cause: `Event` is 40 bytes (the `Error` variant's inline payload sizes every
variant); skin38 emits 1.56M events = 62.5 MB written, re-read, and re-scanned,
grown from a zero-capacity Vec. Plus ~77K `SmallVec<[Child; 4]>` spills per
parse (15-16% of nodes) and 560K per-token pushes into a zero-capacity token
vec. Corpus ratios: ~2.8 events/token, ~0.9 nodes/token, children = tokens +
nodes - 1 exactly.

## 2. Public surface changes

All breaking changes are demotions of internals that `Cst::parse` users never
touch; PR is `perf(ritobin)!`.

```rust
// parse/parser.rs
pub(crate) enum Event { .. }      // was pub; shape changes (section 3)
pub struct Parser<'a> {
    pub text: &'a str,            // unchanged
    pub(crate) tokens: Vec<Token>,   // was pub
    pub(crate) events: Vec<Event>,   // was pub
    // pos, fuel unchanged; new: deferred_errors, opens (section 3)
}
// Parser::open_before is deleted (section 4)

// parse/impls.rs - pub grammar fns; only manual drivers of the parser notice
pub fn stmt_or_list_item(p: &mut Parser, wrap_class: bool) -> (MarkClosed, TreeKind);
// file() passes false; block() passes true
```

Unchanged: `Cst::parse`, `parse_with_config`, `Parser::new`, `build_tree`,
`ErrorPropagation`, `MarkOpened`/`MarkClosed`, every `Cst`/`Node`/`Child` type,
serde formats.

## 3. Event shrink and side-channel errors

```rust
// parse/parser.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    Open(Kind),   // node kind; fieldless enum, so Event packs to 1-2 bytes
    Close,
    Advance,
    Error,        // payload lives in deferred_errors, consumed in order
}

pub(crate) struct DeferredError {
    pub kind: ErrorKind,
    pub span: Option<Span>,
}
```

- `Parser` gains `deferred_errors: Vec<DeferredError>` and `opens: u32`.
  `report`/`advance_with_error` push `Event::Error` plus one side entry; the
  replay keeps a cursor and pops in order (event order = error order, trivially).
- `opens` increments at the two `Open` push sites (`open_raw`, `scope`);
  `build_tree` drops its full-vec pre-count scan and sizes
  `nodes: with_capacity(opens)`, `children: with_capacity(tokens + opens - 1)`
  (the formula is exact: every token is one child, every non-root node is one).
- `Parser::new` pre-sizes `events: with_capacity(tokens.len() * 3)`
  (measured 2.79; a hint, not a cap).
- `scope`/`close` still retype in place (`events[m.index] = Event::Open(kind)`).

Event traffic for skin38: 62.5 MB -> ~1.6-3.1 MB, no growth reallocs, no
pre-count scan.

## 4. Kill `events.insert` (`open_before`)

`block()` is the only caller, post-wrapping `Class` list items via an O(tail)
insert. Instead the `Class` arm pre-opens when asked:

```rust
// stmt_or_list_item, (Name | HexLit, LCurly) arm
let outer = p.open();                              // comment-eating open, as today
let inner = wrap_class.then(|| p.open_raw());      // raw open, no comment logic
p.advance();
let b = block(p);
p.close(b, TreeKind::Block);
match inner {
    Some(class_m) => {
        p.close(class_m, TreeKind::Class);
        (p.close(outer, TreeKind::ListItem), TreeKind::ListItem)
    }
    None => (p.close(outer, TreeKind::Class), TreeKind::Class),
}
```

`open_raw` is today's inner `fn open` hoisted to a `pub(crate)` method. Event
order is identical to the insert path (verified by hand against the comment
case: a leading comment closes before the `ListItem` open in both worlds), so
the tree is unchanged. `open_before` and the `Event` insert go away; `block()`
stops inspecting the returned kind.

Also folded in here: `stmt_or_list_item` matches on `(nth(0), nth(1), nth(2))`
but no arm reads the third element - match on two.

## 5. Token move and the shared scratch stack

```rust
// build_tree, setup
let tokens_len = self.tokens.len();
let mut cst = Cst {
    nodes: Vec::with_capacity(self.opens as usize),
    children: Vec::with_capacity(tokens_len + self.opens as usize - 1),
    tokens: self.tokens,          // moved wholesale; was per-token push
    errors: vec![],
};
let mut next_token: u32 = 0;      // replaces the peekable iterator
```

`Advance` reads `cst.tokens[next_token]`, emits `Child::Token(TokenId(next_token))`,
increments. Error-span peeking is `cst.tokens.get(next_token as usize)`; the
`last_token` capture reads `cst.tokens.last()`. End assert:
`next_token as usize == cst.tokens.len()`.

Per-node buffers become frames on two shared scratch vecs:

```rust
struct StackItem {
    idx: NodeId,
    children_start: u32,   // frame base in scratch_children
    errors_start: u32,     // frame base in scratch_errors
    start_known: bool,     // span bookkeeping unchanged; step 4 deletes it
}
// build-local: scratch_children: Vec<Child>, scratch_errors: Vec<Error>
```

- `Open`: push `StackItem` with the current scratch lengths.
- `Advance`: `scratch_children.push(Child::Token(..))`.
- `Close`: copy `scratch_children[children_start..]` into `cst.children` (one
  contiguous extend, same order as today), truncate, then push
  `Child::Tree(idx)` onto the parent's frame (the new scratch top). Errors by
  propagation mode:
  - `None`: flush the frame into `cst.errors` as the node's range; truncate.
  - `Move`: assign the node an empty range and *leave the entries in scratch* -
    they are now inside the parent's frame. (Today's append, for free.)
  - `Clone`: copy the frame into `cst.errors` for the node; leave the originals.
- Root close after the loop: flush its frame the same way.

This eliminates every `SmallVec` and its spills; scratch length is bounded by
the open path's accumulated children (tiny) and is reused across the parse.

## 6. Invariant and acceptance

**Invariant: the produced `Cst` is bit-identical to `main`'s** - same `nodes`,
`children`, `tokens`, `errors` vectors in the same order (child flush order =
today's close order; token order = lexer order; given the step-0 `push_errors`
fix on both sides). During development, a throwaway differential test parsing
the sample corpus with both builders should assert this; it does not ship.

This invariant belongs to this PR alone. Span-model step 4 changes node span
values on purpose (span doc 4.2), so it cannot carry the invariant forward and
must expect snapshot churn instead.

Acceptance for the PR:

1. `benches/parse.rs` `parse` group: >= 2.5x on big.rito and skin38.rito vs the
   baseline table above; `build_bin` group unchanged.
2. Full crate suite, snapshot tests (zero churn), and `tests/no_panic.rs` pass.
3. `cargo clippy --all-targets` clean.

Suggested commits (subject-only, per repo style):

1. `perf(ritobin)!: shrink parse events and defer error payloads`  (section 3 + nth(2))
2. `perf(ritobin): pre-open class list items instead of event insertion`  (section 4)
3. `perf(ritobin): move tokens wholesale and reuse child scratch frames`  (section 5)

## 7. Out of scope

- Span bookkeeping in the replay loop (`start_known`, `split_at_mut` widening) -
  deleted by span-model step 4, which rebases onto this.
- `Token` packing to 8 bytes - span doc section 6, still gated on being
  memory-bound *after* this lands. Section 6.2 now sizes it and drops the relex
  escape it used to need. The gate now has an experiment. Span doc 11.4 measures
  `Cst::parse` falling from 121 MiB/s to 51 MiB/s while the working set grows
  180x, which supports the premise, but the doubling reallocs this PR removes are
  a confound with the same shape. **Rerun the span doc's
  `entry_reparse/whole_file` group after this PR lands.** A curve that stays
  sloped confirms the premise. A curve that flattens says the reallocs caused it
  and the packing work loses its rationale.
- Removing the `Newline` tokens - span doc section 6, item 1. It cuts 10.5% to
  15.8% of the tokens and their events, so it is the larger win. It also changes
  the token count, which breaks the section 6 bit-identity invariant, so it cannot
  ride with this PR.
- Dropping the event stream for direct arena building - rejected for now; it
  forfeits the mark/retype flexibility error recovery is built on. Revisit only
  if the profile still points at the replay after this list.
- Stage 2 incremental splice - owns the per-keystroke budget for multi-MB files
  regardless of constant factors. Note for whoever takes it: the splice wants
  `ErrorPropagation::None`, because `Move` puts every error on the root and
  leaves a damaged entry with no handle on its own errors (span doc 5.2). This
  PR keeps today's `Move` semantics and does not settle that choice.
