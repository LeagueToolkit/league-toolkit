# ADR-0023: Canonical print

- **Status:** Accepted
- **Date:** 2026-10-02
- **Crates:** `ltk_ritobin`
- **Related:** ADR-0022, #242, the format as `ltk_ritobin::print::canonical` states it
  (`crates/ltk_ritobin/src/print/canonical.rs`)

## Context and problem statement

`ltk_ritobin` can print a bin as the text `ritobin_cli -k -i bin -o text` writes, byte for byte.
Eight per-patch megabins, 8.20 to 16.19, print byte-identical to `ritobin_cli` `368b413`.

A consumer that stores, hashes or diffs ritobin text needs that text to be the same bytes for the
same bin in every release. The census compares builds by the text of their bins.

`Print` is the general printing surface: `print`, `print_with_config` and their writer forms.
Its layout differs from C++ ritobin's (unpadded hashes, `{ }`, short lists on one line,
`map[hash, embed]`), `print_with_config` names hashes from a hash table, and callers depend on
that output as it is. A required method added to `Print` breaks every implementor of it outside
the crate.

## Decision drivers

- One printing call whose bytes are a contract.
- `Print` keeps its behaviour and its shape.
- The contract leaves nothing to configure: no hash table, no indent.

## Considered options

1. **A `PrintCanonical` trait**, with `print_canonical` and `print_canonical_to_writer`, beside
   `Print`.
2. **`print_canonical` as a required method on `Print`.**
3. **`print` switched to the canonical text.**
4. **A canonical mode in `PrintConfig`**, honoured by `print_with_config`.

## Decision

**`PrintCanonical::print_canonical` and `print_canonical_to_writer` print the canonical text, take
no configuration, and hold their output stable: a change to it is a breaking change.** `Print`
keeps its own layout, and makes no promise about it.

## Consequences

- **Positive:** a stable text has a name a caller can depend on. `Print`, its output and its
  implementors are untouched.
- **Negative:** two layouts of the same text in one crate, and a caller picks one by trait. The
  canonical text is fixed at what `ritobin_cli -k` writes, and a fix to it is a breaking release.
- **Revisit when:** C++ ritobin changes its text format, or a consumer needs stable named text.

## Pros and cons of the options

### A `PrintCanonical` trait

- Good: the contract is visible in the name, and it has no input that changes its bytes.
- Good: adding a trait breaks no caller and no implementor.
- Bad: a second trait to import, and a second layout to maintain.

### A required method on `Print`

- Good: one trait.
- Bad: a breaking change for every implementor of `Print` outside the crate.

### `print` switched to the canonical text

- Good: no new surface, and one layout.
- Bad: every caller of `print` sees new output, and `print`'s layout is then frozen for every
  caller, including those who print for a reader.

### A canonical mode in `PrintConfig`

- Good: one entry point.
- Bad: the other fields of the config (indent, hashes, wrap) either break the contract or are
  silently ignored under the mode.
