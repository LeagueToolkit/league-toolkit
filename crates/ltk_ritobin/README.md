# ltk_ritobin

Parser, printer, and concrete syntax tree for the **Ritobin** text format: the human-editable counterpart to League of Legends `.bin` files.

`ltk_ritobin` sits on top of [`ltk_meta`] (the binary `.bin` reader/writer) and provides everything needed to round-trip between bytes on disk and a textual form you can read, diff, and hand-edit. It is also designed as a foundation for editor tooling: every node carries a precise span, parsing is resilient to errors, and there is a visitor API for tree walks. The experimental [`ritobin-lsp`](https://github.com/alanpq/ritobin-lsp) language server is built on this crate.

## What is Ritobin?

Ritobin is a textual representation of the binary property tree used by League of Legends. A trivial file looks like this:

```
#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = { }
entries: map[hash, embed] = { }
```

It maps losslessly to an [`ltk_meta::Bin`], which in turn maps losslessly to the binary `.bin` format the game consumes.

## Pipeline

```
.bin bytes  <──►  ltk_meta::Bin  <──►  Cst  <──►  ritobin text
                    (semantic)       (syntactic)
```

There are two trees in play, and picking the right one matters:

- **`Cst`**: concrete syntax tree. Lossless, carries spans and comments, retains best-effort structure even when the input is malformed. This is what editors and pretty-printers want.
- **`ltk_meta::Bin`**: semantic tree. What the game actually consumes. This is what you want for transformations, validation, and binary I/O.

`Cst::build_bin` bridges from syntactic to semantic. The [`Print`] trait (implemented directly on `Bin`) bridges back to text without needing to round-trip through `Cst`.

## Getting started

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
ltk_ritobin = "0.2"
ltk_meta = "0.5"
```

Doctested, runnable walk-throughs live in the crate-level documentation on docs.rs:

- [Parse ritobin text and build a `Bin`](https://docs.rs/ltk_ritobin/latest/ltk_ritobin/#quick-start)
- [Convert a binary `.bin` to ritobin text with human-readable hashes](https://docs.rs/ltk_ritobin/latest/ltk_ritobin/#converting-bin-to-ritobin)
- [Resilient parsing & error reporting](https://docs.rs/ltk_ritobin/latest/ltk_ritobin/#error-reporting)

Two runnable binaries ship with the crate:

```bash
# .bin → ritobin text (HASH_DIR is optional; defaults to the current directory)
HASH_DIR=hashes/ cargo run -p ltk_ritobin --example bin_to_rito -- input.bin output.rito

# ritobin text → .bin
cargo run -p ltk_ritobin --example rito_to_bin -- input.rito output.bin
```

See [`examples/bin_to_rito.rs`](examples/bin_to_rito.rs) and [`examples/rito_to_bin.rs`](examples/rito_to_bin.rs) for the source.

## Editor / LSP features

- **Spans everywhere.** Every token and tree node carries a `parse::Span { start: u32, end: u32 }`, sufficient for LSP diagnostics, semantic tokens, and go-to-definition.
- **Visitor API.** `cst::visitor::Visitor` (with `enter_tree` / `exit_tree` / `visit_token`) walks a `Cst` for hover, find-references, semantic highlighting, and similar features.
- **Hash resolution.** The `HashProvider` trait (with the ready-made `HashMapProvider`) lets tooling resolve hashes back to source names, both for printing and for surfacing readable identifiers in editor UI.
- **Configurable printer.** [`print::PrintConfig`] and `print::WrapConfig` control indent size, line width, and whether lists / structs may be inlined. Defaults: 4-space indent, 120-column wrap, inline lists, block-form structs.

If you're building tooling on top of this crate, [`ritobin-lsp`](https://github.com/alanpq/ritobin-lsp) is a real-world reference implementation.

## Public API at a glance

- **Syntax tree**: `Cst`, `cst::Kind`, `cst::Child`, `parse::Token`
- **Entry points**: `Cst::parse`, `Cst::build_bin`
- **Printing**: `Print` trait (implemented for `ltk_meta::Bin`), `print::PrintConfig`, `print::WrapConfig`, `print::PrintError`
- **Hashes**: `HashProvider` trait, `HashMapProvider`
- **Parse errors**: `parse::Error`, `parse::ErrorKind`, `parse::Span`, `parse::ErrorPropagation`
- **Tree walking**: `cst::visitor::Visitor`
- **Modules**: `cst`, `parse`, `print`, `typecheck`, `types`, `hashes`

## Feature flags

- `serde`: derives `Serialize`/`Deserialize` on public types and forwards the feature to `ltk_meta`.

## Related crates

- [`ltk_meta`](../ltk_meta): binary `.bin` reader/writer; the semantic layer this crate sits on top of.
- [`ltk_hash`](../ltk_hash): FNV-1a hashing used for object/property names.
- [`league-toolkit`](../../): umbrella crate that re-exports everything behind feature flags.

## License

Licensed under either of MIT or Apache-2.0 at your option.
