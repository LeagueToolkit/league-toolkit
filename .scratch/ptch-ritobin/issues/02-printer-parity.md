---
issue: 242
title: "Ritobin printer: byte parity with moonshadow ritobin"
labels: crate:ltk_ritobin, enhancement, format:bin, area:writing, blocked
---

Print ritobin text byte-for-byte as moonshadow's ritobin prints it, as PRD-001 FR-6 and AC-3 require for every shipped patch. The text shape is `ptch-property-patches.md` [section 15](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s15). #238 parses and prints `PTCH` in `ltk_ritobin`'s own layout; this slice is the parity with moonshadow's.

## Proposed surface

None in the spec. [section 15](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s15) states the requirement and no API for it, and the surface is a spec change before it is this ticket's. The two shapes on the table:

- The default layout becomes moonshadow's. Existing fixtures pin the default, the crate-level doctest among them (`linked: list[string] = { }`, `entries: map[hash, embed] = { }`).
- `PrintConfig` gains a layout that matches moonshadow's, and the default stays as it is.

## Evidence

moonshadow's layout is [`bin_io_text_write.cpp`](https://github.com/moonshadow565/ritobin/blob/d4b8764939d141c1db3ffd186d49bf60fd889b87/ritobin_lib/src/ritobin/bin_io_text_write.cpp) at `d4b8764`. It has no line width: every non-empty list, map, option, pointer and embed prints one item per line, and vectors and colors print inline as `{ 0, 1 }`.

Compared with `ritobin_cli -k` (hashes as hex) against `PrintConfig::default()`:

| fixture | diff lines |
| ------- | ---------- |
| `lolminimap_uiflipped.ptch.bin` | 37 |
| `lolminimap_uibase.bin` | 481 |

The differences:

- A hash prints zero-padded to 8 or 16 digits in moonshadow (`0x0202c6c9`), unpadded here (`0x202c6c9`). On `lolminimap_uiflipped.ptch.bin` this is 6 record keys.
- A map type prints `map[hash,embed]` in moonshadow, `map[hash, embed]` here.
- An empty block prints `{}` in moonshadow, `{ }` here.
- A list of scalars prints one item per line in moonshadow. Here it prints on one line with a trailing comma inside the braces: `{ 0x1ed62b1, 0x68118736, }`. The CST builder emits a list's items under `Kind::TypeArgList`.
- moonshadow ends the file with a newline, and this printer does not.
- moonshadow writes a `"`, a `\` and a control character inside a string as an escape (`\"`, `\\`, `\n`, `\x01`) and reads it back ([`bin_strconv.cpp`](https://github.com/moonshadow565/ritobin/blob/d4b8764939d141c1db3ffd186d49bf60fd889b87/ritobin_lib/src/ritobin/bin_strconv.cpp#L190)). `ltk_ritobin` neither writes nor reads an escape: a backslash stays in the value, and a record path holding a `"` prints in single quotes. moonshadow's `"Lookup{\"weapon\"}"` reads in `ltk_ritobin` as a path with backslashes, which `PropertyPath::new` refuses.

Blocked by #238, the `PTCH` printer. The `PROP` fixtures depend on nothing.

- [ ] [section 15](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s15) names the layout surface, and an ADR records the choice between the two shapes above.
- [ ] `lolminimap_uiflipped.ptch.bin`, `lolminimap_uibase.bin` and `leona_small.bin` print byte-identical to `ritobin_cli -k` at `d4b8764`, compared against golden files committed beside the fixtures.
- [ ] A patch with a non-empty `deleted` root prints as moonshadow's text plus the trailing `deleted` root, in a toolkit-only fixture.
- [ ] Every golden file parses back to the bin it was printed from.
- [ ] A record path with a `{"key"}` subscript prints as moonshadow prints it and parses back.
