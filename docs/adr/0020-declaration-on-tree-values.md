# ADR-0020: Declaration on tree values

- **Status:** Accepted
- **Date:** 2026-09-17
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-12), ADR-0003, ADR-0014, supersedes ADR-0018, #225,
  `docs/design/value-walk.md` [section 3](../design/value-walk.md#s3)

## Context and problem statement

A value's header declares its kind, the item kind of a container or optional, the key and value
kinds of a map, the class of a `Struct` or `Embedded`, and how many items it holds. An empty
container declares all of these with no item to read them from.

ADR-0018 exposes the declarations through `RawValue::value_view()`, on the view only. Its revisit
condition is generic visitors that require them in the shared tree contract.

`ltk-manager` runs its type checks and its object index as visitors generic over both trees. Each
reads item kinds, key kinds, a pointer's class with 0 for the null pointer, whether an optional
holds a value, and item counts, with an optional counted as 0 or 1. The manager holds a `Declared`
trait of five methods, implemented once per tree, and its `bin_property_type`, `object_index`,
`vfx_random`, `bin_resolver_key_loss` and pass modules bound on it.

Inside `ltk_meta`, `ValueShape::of`, `PropertyView::shape` and `item_count`, both `can_contain_node`
implementations and the merge's shape check each compute part of the same answer.

## Decision drivers

- A visitor generic over both trees reads every declaration without naming either tree.
- The tree contract stays small.
- Over a view, a declaration reads a header and no leaf, item, entry or property.
- The patch type rule compares a pointer without its class (ADR-0003).

## Considered options

1. **Borrowed view access only** - ADR-0018 as accepted: `value_view()` on `RawValue`.
2. **One declaration method** - `TreeValue::declaration` returns a `Declaration` value.
3. **One method per declaration** - `item_kind`, `key_kind`, `class_hash`, `is_empty_option` and
   `item_count` on `TreeValue`.
4. **A wider `ValueShape`** - `TreeValue::shape` returns `ValueShape` with a count and a pointer's
   class added.

## Decision

**Option 2. `TreeValue::declaration` returns a `Declaration`: the kind, item kind, key kind,
class and count a value's header declares.** A `Declaration` records a pointer's class, 0 for the
null pointer, and counts an optional as 0 or 1. `can_contain_node` is read off it. `ValueShape`
converts from a `Declaration` without the count and without a pointer's class.
`RawValue::value_view()` remains the borrowed streaming access. The surface is
`docs/design/value-walk.md` [section 3](../design/value-walk.md#s3).

## Consequences

- **Positive:** one method replaces the manager's five-method trait and its two implementations.
  `can_contain_node`, `ValueShape::of` and the merge's shape check read one answer per tree.
- **Negative:** an optional counts as 0 or 1 in a `Declaration`, and `PropertyView::item_count`
  answers `None` for one. Over a view, a declaration raises the header checks the layout core
  raises, `InvalidNesting` and `InvalidKeyType` included.
- **Revisit when:** a check needs a declaration a header does not carry, or the value model loses
  `M` and the trees collapse toward one representation (ADR-0014).

## Pros and cons of the options

### Option 1: borrowed view access only

- Good: no change to the tree contract.
- Bad: a generic visitor cannot reach a declaration. Every consumer writes one adapter per tree,
  and the manager's `Declared` is that adapter.

### Option 3: one method per declaration

- Good: a visitor asks for exactly the field it reads.
- Bad: five fallible methods on a sealed trait, each implemented twice, and each re-reading the
  header over a view. A sixth declaration is a sixth method.

### Option 4: a wider `ValueShape`

- Good: one type for a value's shape across the crate.
- Bad: `ValueShape` is the patch type rule. A count and a pointer's class inside it make
  `ValueShape::matches` differ from `==`, and change its `Display` and serde form, both released.
