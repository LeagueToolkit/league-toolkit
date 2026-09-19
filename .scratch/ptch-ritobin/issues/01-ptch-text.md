---
issue: 238
title: "Ritobin PTCH: parse and print patch files"
labels: crate:ltk_ritobin, enhancement, format:bin, area:reading, area:writing, area:api
---

Add PTCH parsing and printing to `ltk_ritobin`, as specified by `ptch-property-patches.md` [section 15](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s15) and PRD-001 FR-6. Byte parity with moonshadow's printer, which AC-3 measures, is #242. The consumer is declarative `.rito` overrides in [league-mod#191](https://github.com/LeagueToolkit/league-mod/issues/191), part of [game-data roadmap step 1](https://wiki.leaguetoolkit.dev/reference/mod-packages/game-data/#roadmap).

## Proposed surface

The surface in section 15 is `RootKind::Patches` and `RootKind::Deleted`, a `Cst::build` entry point returning `(BinFile, Vec<DiagnosticWithSpan>)`, and `Print` implementations for `BinOverride` and `BinFile`. `Cst::build_bin` retains its signature and diagnoses a PTCH input. A caller requiring valid binary output rejects conversion diagnostics; a best-effort PROP is not a successful PTCH conversion.

The text representation is moonshadow's `patches: map[hash,embed]`, with each record represented by a `patch` embed containing `path: string` and a typed `value`. This synthetic example has two records for the same object and property:

```text
#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
patches: map[hash,embed] = {
    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 0, 1 }
    }
    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 1, 0 }
    }
}
```

`entries` carries the patch's added objects. `patches` carries an ordered sequence of records, including repeated object keys and repeated paths. The outer mapping must not collapse into an `IndexMap` keyed by object hash.

## Evidence and invariants

- Moonshadow's [binary reader, lines 188-215](https://github.com/moonshadow565/ritobin/blob/d4b8764939d141c1db3ffd186d49bf60fd889b87/ritobin_lib/src/ritobin/bin_io_binary_read.cpp#L188) appends each record to the map's pair sequence and exposes its path and typed value. Its [binary writer](https://github.com/moonshadow565/ritobin/blob/d4b8764939d141c1db3ffd186d49bf60fd889b87/ritobin_lib/src/ritobin/bin_io_binary_write.cpp#L189) emits that sequence in order; an absent `patches` section writes zero records.
- The toolkit's PTCH authoring contract requires `version: u32 = 3` and an empty `linked` list. These restrictions come from section 15, not from an assumption that moonshadow's parser enforces them. Printing retains empty `entries` and `linked` sections.
- `deleted: list[hash]` is the toolkit extension defined in section 15. It maps to `BinOverride::deleted` and is omitted when empty. Nonempty deletion lists use toolkit round-trip fixtures; compatibility fixtures with moonshadow are #242.
- Record paths use the existing `PropertyPath` grammar and validation. Their text remains authored; printing does not unhash or rename path members. Record value kinds remain authored, without schema coercion.
- Diagnostics identify malformed record keys, embed values, required fields, path strings and typed values at their source spans. The complete conversion preserves the distinction between PROP and PTCH. PTCH-only roots on a PROP produce diagnostics rather than disappearing.
- The conversion uses `ltk_meta`'s binary reader/writer and PTCH model. This work adds no second PTCH wire codec or application engine.

Blocked by: none. The binary PTCH model and codec are available. PR #227 and issue #237 provide traversal and mutation for the downstream materializer; this text-format slice has no dependency on them.

- [ ] Parse the example into an override with exactly two records, preserving both object hashes, paths, value kinds and order.
- [ ] Parse, print and reparse a PTCH containing added objects and heterogeneous record values; its `BinOverride` is equal after the round trip.
- [ ] Convert PTCH text to binary and read it with `ltk_meta`; print binary PTCH to text and parse it back without losing records or objects.
- [ ] Preserve repeated object keys, including two records for the same path; applying the compiled patch to a fixture follows the authored order.
- [ ] Parse an absent or empty `patches` section as zero records, matching the referenced writer's empty-patch behavior.
- [ ] Round-trip nonempty toolkit `deleted` lists, and omit `deleted` when empty. Shared-format fixtures contain no toolkit-only deletion extension.
- [ ] Diagnose nonempty PTCH links, unsupported PTCH text versions, malformed record shape, missing or duplicate `path`/`value` fields, non-string paths, invalid property paths and invalid typed values with source locations.
- [ ] A checked caller cannot accept malformed PTCH as a partial PROP or as a successful patch with silently discarded records; `build_bin` diagnoses PTCH and retains its PROP behavior.
- [ ] Existing PROP parse/print fixtures remain unchanged; PTCH-only roots on PROP are diagnosed.
