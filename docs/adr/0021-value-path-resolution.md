# ADR-0021: Value path resolution

- **Status:** Accepted
- **Date:** 2026-09-23
- **Crates:** `ltk_meta`
- **Related:** ADR-0005, league-mod ADR-0028,
  `docs/design/ptch-property-patches.md` [section 9](../design/ptch-property-patches.md#s9),
  `docs/design/value-walk.md` [section 4](../design/value-walk.md#s4)

## Context and problem statement

`resolve`, `resolve_mut` and `patch` take a `PropertyPath`. A `PropertyPath` segment names a
property by the FNV-1a hash of its text. A bin stores name hashes only, and a field whose
plaintext is unknown has no `PropertyPath`.

`league-mod` game-data declarations edit a live bin through `Bin::patch`, and read the game's
copy of an entry through `BinObject::resolve`. The declaration standard spells a field with no
known name as `0x` and eight hexadecimal digits. `0x1234abcd` is a legal property name, and the
client hashes it as text (`MetaPath_resolve`, [section 8.1](../design/ptch-property-patches.md#s8.1)).

ADR-0005 keeps `PropertyPath` the client's language, with no hash escape, and makes `ValuePath`
the hash-addressed type. A `ValuePath` addresses a field by hash and a map entry by a `MapKey` of
the map's key kind. No operation resolves a `ValuePath`.

## Decision drivers

- A field with no known plaintext is readable and writable through the crate.
- `PropertyPath` resolves the way the client resolves it, with no second reading of its text.
- One traversal rule and one type rule for every address.
- `ltk_meta` performs every write to a tree (league-mod ADR-0017).

## Considered options

1. **`ValuePath` resolution.** `resolve_at`, `resolve_at_mut` and `patch_at` take a `ValuePath`,
   beside each `PropertyPath` operation.
2. **A naming strategy.** `resolve_with` and `patch_with` take a `PropertyPath` and a rule that
   reads a `0x` segment as its hash.
3. **A hash escape in `PropertyPath`.** A path parsed in a declaration mode reads a `0x` segment as
   its hash.
4. **A walk in the consumer.** `league-mod` walks and writes the tree by hash itself.

## Decision

**A `ValuePath` resolves and patches through `resolve_at`, `resolve_at_mut` and `patch_at`, by
the traversal and type rules of a `PropertyPath`.** `docs/design/ptch-property-patches.md`
[section 9](../design/ptch-property-patches.md#s9) states the operations and
`docs/design/value-walk.md` [section 4](../design/value-walk.md#s4) the key conversion.

`MapKey::from_literal` converts a `{key}` literal to the key of a map's key kind. A consumer
holding text with an escape builds a `ValuePath` from it; `ltk_meta` holds no escape.

## Consequences

- **Positive:** a nameless field is readable and patchable, and `PropertyPath` keeps one meaning.
- **Positive:** a `ValuePath` from a walk, a diff or a merge report resolves back to its position.
- **Negative:** fourteen new methods, one beside each `PropertyPath` operation.
- **Negative:** a `ResolveError` segment index counts a field and its subscript apart for a
  `ValuePath` and together for a `PropertyPath`.
- **Negative:** a consumer converting a `{key}` literal needs the map's key kind before the walk
  reaches the map.
- **Revisit when:** the client resolves a path by hash.

## Pros and cons of the options

### `ValuePath` resolution

- Good: ADR-0005's hash-addressed type gains the operations; `PropertyPath` is untouched.
- Bad: a second family of methods, and a key literal converted before the walk.

### A naming strategy

- Good: the literal key conversion stays inside the walk; the consumer keeps one path type.
- Bad: one `PropertyPath` text resolves two ways. A `0x1234abcd` property name and the hash
  `0x1234abcd` are two fields, and the call site decides which.

### A hash escape in `PropertyPath`

- Good: no new method; every existing operation reads the escape.
- Bad: ADR-0005's rejected option. A path carrying the escape is text the client hashes as text,
  and a `PTCH` record holding it resolves to another field.

### A walk in the consumer

- Good: no change in `ltk_meta`.
- Bad: a second copy of the traversal and type rules in `league-mod`, and writes outside the
  format crate.
