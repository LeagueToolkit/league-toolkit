# ADR-0012: Annotations are a side table keyed by span, not a type parameter on `Ast`

- **Status:** Accepted
- **Date:** 2026-10-20
- **Crates:** `ltk_ritobin`
- **Related:** #276, and the spec sections that state the rules:
  `docs/design/ritobin-annotations.md` [section 5](../design/ritobin-annotations.md#s5),
  [section 6](../design/ritobin-annotations.md#s6) and
  [section 8](../design/ritobin-annotations.md#s8)

## Context and problem statement

Ritobin text gains `@name(args)` lines that apply to the entry or list item after them. The crate
gives them no meaning: a patch tool, an LSP and a generator each read the same file and each owns
some of the names. So the `Ast` has to carry the raw annotations, and a consumer has to be able to
turn them into a type of its own and keep that typed form next to the tree it came from.

The first draft of #276 did this with a type parameter: `Ast<A = Annotation>`, with
`map_annotations` and `try_map_annotations` producing an `Ast<T>`. Only the `annotations` field
used `A`. `roots`, `diagnostics`, the visitor, `to_bin` and the path queries never touched it, yet
every `impl Ast` became `impl<A> Ast<A>`, `VisitorExt::walk` and `AstPathIter::from_ast` grew an
`<A>`, and any downstream `fn f(ast: &Ast)` silently meant `Ast<Annotation>` and rejected an
`Ast<T>`.

`ltk_meta` has a type parameter in the same position, `M = NoMeta` (ADR-0010). The difference is
that `M` is in every node of that tree, so every signature over it genuinely varies by `M`. Here
the parameter touches one field.

## Decision drivers

- `Ast` is a syntax tree; it should stay a plain type that every consumer names the same way.
- A consumer that resolves annotations must be able to keep the typed result beside the `Ast`.
- Several tools read one file; resolving for one must not consume or alter the raw table.
- A choice that is cheap to extend later beats one that is cheap to use today, because removing a
  type parameter is a breaking change and adding one is not.

## Considered options

1. **`Ast<A = Annotation>`** - the typed form replaces the raw one inside the tree.
2. **A side table `Annotations<A>` keyed by target span, with `Ast` holding `Annotations` and a
   consumer holding `Annotations<T>` beside it** - the generic lives on the one type that varies.
3. **Annotations stored inside the nodes** - `Property`, `MapEntry` and `RootEntry` each gain an
   `annotations` field; the visitor sees them for free.

## Decision

**Option 2. `Ast` is not generic. It holds `Annotations`, the raw table; `resolve` borrows that
table and returns `Resolved<T, E>`, which a consumer stores wherever it stores the `Ast`.**

[Section 5](../design/ritobin-annotations.md#s5) states the data model,
[section 6](../design/ritobin-annotations.md#s6) the queries and
[section 8](../design/ritobin-annotations.md#s8) resolution. Attachment is one pass over the CST
before any value is resolved, so every scope follows one rule and the value resolvers never see an
annotation.

## Consequences

- **Positive:** every signature over `Ast` stays as it was. The generic appears once, on
  `Annotations<A>`, and only consumers that resolve ever write it. Two tools resolve the same table
  independently. The typed form can be dropped or rebuilt without touching the tree.
- **Negative:** a consumer that caches the typed form keeps two values, `Ast` and `Annotations<T>`,
  in a struct of its own instead of one `Ast<T>`. Lookup is by span, not by walking into a node, so
  a visitor that wants annotations carries the table alongside and queries it per node.
- **Negative:** target spans must equal the AST node's own span for a lookup by node to work. The
  attachment pass keys on the trimmed CST span, and a node kind whose AST span diverges from it is a
  bug the attachment tests exist to catch.
- **Revisit when:** a second consumer needs annotations on a node kind that has no span of its
  own, or the visitor needs them without a lookup. Both argue for option 3, which can be added
  without removing the table.

## Pros and cons of the options

### Option 1: `Ast<A = Annotation>`

- Good: one value carries tree and typed annotations; `ast.annotations` is already the consumer's
  type.
- Bad: a parameter that varies one field costs every signature over the type. The default makes
  `fn f(ast: &Ast)` compile and then reject an `Ast<T>`, so each new function has to remember
  `<A>`. Errors from mapping had to be squeezed into `Diagnostic`, which is `Copy` and cannot hold a
  consumer's error type. Nothing in the crate or its tests ever built an `Ast<T>` with
  `T != Annotation`.

### Option 2: side table keyed by span

- Good: the generic is on the one type that varies; everything else is unchanged. Raw and typed
  tables answer the same queries. Resolution borrows, so it composes across tools.
- Bad: two values to keep together, and a span lookup per node instead of a field read.

### Option 3: annotations inside the nodes

- Good: the visitor and any consumer holding a node see its annotations with no lookup.
- Bad: list items are bare `Value`s with no slot for a field, so they would need a wrapper type or
  a side table anyway. It changes several public structs for the tempting case, a typed annotation
  on a property, and still needs `Annotations<T>` for the rest. It can be layered on later if the
  visitor case turns out to matter.
