# Annotations in `ltk_ritobin`

The API spec for annotations in ritobin text: the syntax, how an annotation attaches to a node, and
the surface a consumer reads them through.

**This document states what is true now.** Where the code and a section here disagree, the section
is the bug and gets edited.

Implemented in `ltk_ritobin` (#276). The surface lives in `ast::annotation`; the builder side in
`ast::resolve::annotation`; the tests in `tests/annotations.rs`.

## <a id="s1"></a>1. Summary

An annotation is `@name` or `@name(args)` on its own line before an entry or a list item.
`ltk_ritobin` gives it no meaning and it never reaches a `Bin`. The crate parses it, attaches it to
the node that follows, and hands it to a consumer, who gives it meaning by implementing
`FromAnnotation`.

```text
@replace
skinScale: f32 = 1.5
@merge(depth = 2, strict)
mVfx: embed = VfxData { ... }
```

`Ast` holds the raw annotations and is not generic. A consumer's typed form lives beside the `Ast`,
never inside it (D1).

## <a id="s2"></a>2. Vocabulary

- **annotation** - one `@name(args)` as written. Type `Annotation`.
- **argument** - one item inside the parentheses: a **flag** (`strict`) or a **named** argument
  (`depth = 2`). Type `Argument`.
- **scope** - the file root, or the children of one `{ }` block.
- **target** - the node an annotation applies to: the next sibling in the same scope that is
  neither an annotation nor a comment.
- **attached** - an annotation paired with its target's span. Type `Attached<A>`.
- **direct** / **inherited** - an annotation is direct on its target and inherited by every node
  inside the target. `on` returns direct annotations, `applying_to` returns both.
- **source view** - an annotation paired with the source text it was parsed from. Type
  `AnnotationRef`. Annotations store spans, not strings, so reading a name needs the text.
- **resolve** - turn every raw annotation into a consumer type `T`, through `FromAnnotation` or a
  closure. Output `Resolved<T, E>`.

Words not used: **attribute** (Rust's term, and ritobin has none), **decorator**, **metadata**
(taken by `ltk_meta`'s `M` parameter, ADR-0010).

## <a id="s3"></a>3. Syntax

```text
annotation = "@" name [ "(" [ argument { "," argument } [ "," ] ] ")" ] newline
argument   = name [ "=" literal ]
literal    = string | number | hex | "true" | "false" | "null"
```

- An annotation is one line. A newline inside the parentheses ends it with a parse error.
- An annotation stands wherever an entry or a list item can: the file root, struct and embed
  blocks, map blocks, list blocks. Inside a type argument list it is a parse error. Inside a
  list-like block (`vec3`, `rgba`, `mtx44`) it is an `UnexpectedItem` diagnostic: the value is one
  node, so there is nothing inside it to target. The annotation stays in the table keyed to the
  component it preceded, which no node has, so only `iter` sees it.
- Annotations stack. Comments may stand between an annotation and its target.
- Two arguments with one name in one annotation are an error ([section 9](#s9)). The same
  annotation name may repeat on one target; the crate never dedupes (D11).
- A literal is evaluated without an expected type:

| Literal | `Value` |
| --- | --- |
| number | the first of `U64`, `I64`, `F32` that parses it |
| hex | `Hash` if it fits 32 bits, else `WadChunkLink`; never an integer |
| string | `String`, unescaped |
| `true` / `false` | `Bool` |
| `null` | `None` |

## <a id="s4"></a>4. Attachment

- Attachment runs once over the CST inside `Ast::from_cst`, before values are resolved. Value
  resolvers skip annotation nodes.
- An annotation attaches to its target ([section 2](#s2)). An error tree is a valid target: the
  annotation is kept and reachable through `iter`, but no AST node is built for it, so `on` never
  returns it.
- The stored target span is the target's trimmed CST span. It equals `Annotatable::target()` of the
  AST node built from it (D4).
- Annotations with no target before the scope ends produce one `TrailingAnnotations` diagnostic that
  covers them all, and are dropped.
- Order: the table is sorted by target start offset, so targets follow source order and a nested
  target follows the one enclosing it. Annotations on one target keep source order.

## <a id="s5"></a>5. Data model

```rust
// ltk_ritobin::ast
pub struct Ast {
    pub roots: Roots,
    pub annotations: Annotations,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

// ltk_ritobin::ast::annotation

/// One `@name(args)` as written in the source.
#[derive(Debug, Clone)]
pub struct Annotation { /* private */ }

impl Annotation {
    /// From `@` to the closing `)` or the end of the name.
    pub fn span(&self) -> Span;
    /// The name only, without `@`.
    pub fn name(&self) -> Span;
    pub fn arguments(&self) -> &[Argument];
    /// Pairs this annotation with the source it was parsed from.
    pub fn with_source<'a>(&'a self, text: &'a str) -> AnnotationRef<'a>;
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Argument {
    Flag(Span),
    Named { name: Span, value: Value },
}

impl Argument {
    pub fn name(&self) -> Span;
    /// The whole argument: the name, or `name = value`.
    pub fn span(&self) -> Span;
    pub fn value(&self) -> Option<&Value>;
}

/// An annotation and the span of the node it applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attached<A> { /* private */ }

impl<A> Attached<A> {
    pub fn target(&self) -> Span;
    pub fn annotation(&self) -> &A;
    pub fn into_annotation(self) -> A;
}

/// A node an annotation can target. Sealed.
pub trait Annotatable {
    fn target(&self) -> Span;
}
// implemented for Root, RootEntry, Property, MapEntry, Value as a list item,
// and Span as the identity, for a caller that already holds a target span
```

`Root` is a top-level statement (`type`, `version`, `linked`, `entries`, ...); `RootEntry` is one
entry of the `entries` map. Both take annotations.

`Annotations<A>` is in [section 6](#s6). It is the only generic type in the surface (D2).

## <a id="s6"></a>6. Queries

```rust
/// Annotations keyed by target, in the order of [section 4](#s4).
#[derive(Debug, Clone, Default)]
pub struct Annotations<A = Annotation> { /* private */ }

impl<A> Annotations<A> {
    /// Annotations direct on `node`, in source order.
    pub fn on(&self, node: &impl Annotatable) -> impl Iterator<Item = &A>;
    /// Annotations direct on `node` or inherited from an enclosing node: outermost target first,
    /// source order within one target.
    pub fn applying_to(&self, node: &impl Annotatable) -> impl Iterator<Item = &Attached<A>>;
    pub fn iter(&self) -> impl Iterator<Item = &Attached<A>>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl<'a, A> IntoIterator for &'a Annotations<A> { /* Item = &'a Attached<A> */ }

impl Annotations {
    /// Binds the source text once for raw queries.
    pub fn with_source<'a>(&'a self, text: &'a str) -> SourceAnnotations<'a>;
}

/// [`Annotations`] with the source bound. Same queries: `on` yields `AnnotationRef<'a>`,
/// `applying_to` and `iter` yield `Attached<AnnotationRef<'a>>`.
#[derive(Debug, Clone, Copy)]
pub struct SourceAnnotations<'a> { /* private */ }
```

`on` matches a target by its exact span, not by start offset: a map entry and its key start at
the same byte, and an annotation on the entry is on the entry alone. `applying_to` matches every
target whose span contains the node's span, ends included, so the key and the value of an
annotated map entry both inherit it (D4).

```rust
let deprecated = ast.annotations.with_source(text).on(prop).any(|a| a.name() == "deprecated");
```

## <a id="s7"></a>7. Source view and arguments

```rust
/// An [`Annotation`] paired with its source text.
#[derive(Debug, Clone, Copy)]
pub struct AnnotationRef<'a> { /* private */ }

impl<'a> AnnotationRef<'a> {
    pub fn annotation(&self) -> &'a Annotation;
    pub fn name(&self) -> &'a str;
    /// Source text of an argument's name.
    pub fn arg_name(&self, argument: &Argument) -> &'a str;
    /// `Ok(None)` when absent or `name = null`.
    pub fn arg<T: FromArgument<'a>>(&self, name: &str) -> Result<Option<T>, ArgError>;
    /// Like `arg`, but absent and `null` are `Missing`.
    pub fn require<T: FromArgument<'a>>(&self, name: &str) -> Result<T, ArgError>;
    /// `true` for a bare `name` or `name = true`; `false` when absent, `null` or `name = false`.
    pub fn flag(&self, name: &str) -> Result<bool, ArgError>;
    /// Errors on the first argument whose name is not in `allowed`.
    pub fn only(&self, allowed: &[&str]) -> Result<(), ArgError>;
    /// Spans a consumer error at this annotation.
    pub fn error<E>(&self, error: E) -> Spanned<E>;
    /// This one annotation as `T`.
    pub fn resolve<T: FromAnnotation>(&self) -> Result<T, Spanned<T::Error>>;
}

/// A type an argument value converts to. Sealed.
pub trait FromArgument<'a>: Sized {}
```

| `T` | Accepts | Otherwise |
| --- | --- | --- |
| `u8` to `u64`, `i8` to `i64` | `U64`, `I64` in range | `OutOfRange`, `WrongType` |
| `f32` | `F32`, `U64`, `I64` | `WrongType` |
| `bool` | `Bool` | `WrongType` |
| `&'a str` | `String` | `WrongType` |
| `BinHash` | `Hash`; `String`, hashed | `WrongType` |
| `WadHash` | `WadChunkLink`, `Hash`; `String`, hashed | `WrongType` |

A string converts to a hash the way `Value::try_coerce_to` converts a string key to a `hash`
value: lowercased FNV-1a for `BinHash`, lowercased XXH64 for `WadHash`. A flag looked up by `arg` or
`require` is `WrongType`, and so is `flag` on a value that is not `Bool`.

**Panics.** `with_source` and every `AnnotationRef` accessor panic if `text` is not the source the
`Ast` was built from, because a span then falls outside the text or splits a character. This is a
caller bug (D3).

## <a id="s8"></a>8. Resolution

```rust
/// Turns a raw annotation into a consumer type. One impl per tool, on an enum of every
/// annotation that tool reads. Total: an annotation the tool does not own is the consumer's own
/// variant, such as `Unknown`, or its own error.
pub trait FromAnnotation: Sized {
    type Error: std::error::Error;
    fn resolve(annotation: AnnotationRef<'_>) -> Result<Self, Spanned<Self::Error>>;
}

impl Annotations {
    /// `resolve_with(text, T::resolve)`.
    pub fn resolve<T: FromAnnotation>(&self, text: &str) -> Resolved<T, T::Error>;
    /// Resolves through a closure, for a resolver that carries state.
    pub fn resolve_with<T, E, F>(&self, text: &str, resolver: F) -> Resolved<T, E>
    where
        F: FnMut(AnnotationRef<'_>) -> Result<T, Spanned<E>>;
}

impl Ast {
    /// Shorthand for `self.annotations.resolve(text)`.
    pub fn resolve_annotations<T: FromAnnotation>(&self, text: &str) -> Resolved<T, T::Error>;
}

#[derive(Debug, Clone)]
pub struct Resolved<T, E> { /* private */ }

impl<T, E> Resolved<T, E> {
    pub fn annotations(&self) -> &Annotations<T>;
    pub fn errors(&self) -> &[Spanned<E>];
    pub fn into_parts(self) -> (Annotations<T>, Vec<Spanned<E>>);
}

impl<E: From<ArgError>> From<ArgError> for Spanned<E> { /* keeps the ArgError's span */ }
```

- `resolve` borrows the table. Several consumers resolve the same `Ast` independently.
- `resolve_with` is the path for a resolver with runtime state, such as a name table loaded from a
  file. `FromAnnotation` is the path for a fixed type and is the one most consumers want.
- Every annotation resolves or fails; nothing is dropped. A failed annotation is in `errors`
  only. Its target gets no entry in `annotations`. A consumer that wants to flag unknown names,
  an LSP for example, filters its own `Unknown` variant.
- Error spans: an `ArgError` raised with `?` keeps the argument's span. `AnnotationRef::error` uses
  the annotation's span.
- `resolve` does not write to `Ast::diagnostics`. `Diagnostic` is `Copy` and cannot carry a
  consumer's error (D5).

```rust
enum Directive { Replace, Merge { depth: u64, strict: bool }, Unknown }

impl FromAnnotation for Directive {
    type Error = DirectiveError; // has #[from] ArgError

    fn resolve(a: AnnotationRef<'_>) -> Result<Self, Spanned<Self::Error>> {
        match a.name() {
            "replace" => Ok(Self::Replace),
            "merge" => {
                a.only(&["depth", "strict"])?;
                Ok(Self::Merge { depth: a.require("depth")?, strict: a.flag("strict")? })
            }
            _ => Ok(Self::Unknown),
        }
    }
}

let resolved = ast.resolve_annotations::<Directive>(text);
for d in resolved.annotations().on(prop) { /* ... */ }
```

A consumer that caches the typed form stores it next to the `Ast` in a struct of its own.

## <a id="s9"></a>9. Errors

Building the `Ast` reports malformed annotations as `Diagnostic::AnnotationResolve`. The span is
the offending token.

```rust
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[non_exhaustive]
pub enum AnnotationResolveError {
    #[error("missing annotation name")]
    MissingName { span: Span },
    #[error("named argument is missing a value")]
    MissingArgValue { span: Span },
    /// `span` is the second occurrence.
    #[error("duplicate argument")]
    DuplicateArgument { span: Span },
    /// A literal that does not evaluate, a bad escape in a string included.
    #[error("invalid argument value: {0}")]
    ResolveValue(#[from] ValueEvalError),
}

impl AnnotationResolveError {
    pub fn span(&self) -> Span;
}
```

One malformed argument fails the whole annotation: it is dropped and nothing attaches to its
target.

`Diagnostic::TrailingAnnotations(Span)`: annotations with no target ([section 4](#s4)).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind}")]
pub struct ArgError {
    pub span: Span,
    pub kind: ArgErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ArgErrorKind {
    #[error("missing argument")]
    Missing,
    #[error("expected {expected}")]
    WrongType { expected: &'static str },
    #[error("value out of range for {ty}")]
    OutOfRange { ty: &'static str },
    #[error("unexpected argument")]
    Unexpected,
}
```

`Missing` is spanned at the annotation; the name is the one the caller passed to `require`. Every
other kind is spanned at the argument, so the name is `text[span]`. No error owns a string: the
error types are `Copy`, like `Diagnostic` (D15).

## <a id="s10"></a>10. Printing

- An annotation prints on its own line, at the indent of its target.
- Arguments print flat, separated by `, `, with no trailing comma. The list never wraps
  ([section 3](#s3)).
- Printing then parsing gives the same annotations on the same targets.

## <a id="s11"></a>11. Testing

- **Parse:** every form in [section 3](#s3), and every malformed form gives a parse error without
  panicking (the `no_panic` proptest covers `@`).
- **Attachment:** target spans equal `Annotatable::target()` for every node kind in
  [section 5](#s5). Cases: a comment between annotation and target, at the root and in blocks;
  stacked annotations; trailing annotations; a target at end of file with no trailing newline.
- **Queries:** `on` and `applying_to` on nested targets, checked for order; an annotated map
  entry is `on` the entry and neither its key nor its value, and both inherit it; an entry with
  no value is a target.
- **Resolution:** resolved, unknown and failed annotations in one file, through `resolve` and
  `resolve_with`; error spans for each `ArgErrorKind`; one `FromArgument` case per row of the table
  in [section 7](#s7); `null` through `arg`, `require` and `flag`; a duplicate argument name.
- **Round trip:** printed output reparsed and its annotations compared with the original's.

## <a id="s12"></a>12. Rules

| ID | Rule | Instead of | Why | Spec |
| -- | ---- | ---------- | --- | ---- |
| D1 | `Ast` is not generic. Typed annotations live in `Annotations<T>` beside it. | `Ast<A = Annotation>`. | A type parameter that only touches one field costs every `Ast` signature `<A>`. Adding one later is non-breaking; removing it is not. | [section 1](#s1), [section 5](#s5); ADR-0012 |
| D2 | `Annotations<A>` is the one generic type. | A second table type for typed annotations. | Raw and typed tables answer the same queries. | [section 6](#s6) |
| D3 | Annotations store spans. Text is read through `AnnotationRef`, which pairs them with the source. | Owned `Box<str>` names. | The rest of `Ast` is span-based and does not own the source. | [section 5](#s5), [section 7](#s7) |
| D4 | Target spans are trimmed CST spans. `on` matches the exact span; `applying_to` by inclusive containment. | Untrimmed spans; matching by start offset; half-open containment. | Untrimmed spans include trailing whitespace and never equal an AST node's span. A map entry and its key share a start offset, so start alone would put the entry's annotation on the key. | [section 4](#s4), [section 6](#s6) |
| D5 | `resolve` returns errors and never writes `Ast::diagnostics`. | Merging consumer errors into `Diagnostic`. | `Diagnostic` is `Copy`. | [section 8](#s8) |
| D6 | `FromAnnotation` takes `AnnotationRef` and nothing else. New context is added as `AnnotationRef` methods. | Extra trait method parameters. | A new trait parameter breaks every implementation; a new method breaks none. | [section 8](#s8); ADR-0012 |
| D7 | `FromAnnotation` is total: one impl per tool on an enum of its annotations, and an unknown name is the consumer's own variant. | `Result<Option<Self>, _>` with `None` for "not mine" and an `unclaimed` list on `Resolved`. | The `Option` read as one impl per annotation kind. Every annotation resolving to something keeps the decision about unknown names with the consumer: an LSP and a patch tool want different answers. | [section 8](#s8) |
| D8 | Attachment is one CST pass before value resolution. | Per-resolver annotation handling. | One rule for every scope; resolvers do not see annotations. | [section 4](#s4); ADR-0012 |
| D9 | An annotation is one line; arguments never wrap. | Multi-line argument lists. | A newline ends a statement everywhere else in ritobin. | [section 3](#s3), [section 10](#s10) |
| D10 | The public module is `ast::annotation`, beside `ast::hash` and `ast::query`. | A `node::annotation` module glob re-exported into `ast::node`. | `ast::node` holds the tree's nodes. Annotations are a side table keyed by node, not nodes. | [section 5](#s5) |
| D11 | A duplicate argument name is a build error; a repeated annotation name on one target is not. | Last-wins for arguments; deduping annotations. | An argument is a key. An annotation is a statement, and `@tag(a) @tag(b)` is a reasonable thing to write. | [section 3](#s3), [section 9](#s9) |
| D12 | `null` is absent for `arg`, `require` and `flag`. | A `WrongType` error. | Generated text writes `null` where a hand would omit the argument. | [section 7](#s7) |
| D13 | A string converts to a hash. Not optional: an annotation argument is written the way the matching bin value is. | `WrongType`, with the consumer hashing by hand. | A hash key is written as a string everywhere else in ritobin, and `try_coerce_to` already defines the conversion. | [section 7](#s7) |
| D14 | `Resolved<T, E>` has no trait bound; `resolve_with` takes a closure. | `Resolved<T: FromAnnotation>` only. | A resolver with runtime state cannot be a trait impl with no `self`. | [section 8](#s8) |
| D15 | Crate error types carry spans, never strings, and are `Copy`. | `Box<str>` names in `ArgErrorKind`. | The source is at hand wherever an error is rendered; `Diagnostic` already follows this rule. | [section 9](#s9) |
