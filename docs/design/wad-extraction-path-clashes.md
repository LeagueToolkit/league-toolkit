# Path clashes in WAD extraction

Design record for the naming rules in `ltk_wad::extractor`.

Status: implemented, 2026-08-25. Supersedes the write-time-only resolution that shipped in
`ltk_wad` 0.4.0.

## 1. Summary

A WAD is a flat map from a path hash to bytes. A file system is a tree, in which every name is a
file **exclusive-or** a directory. The projection from the one to the other is not injective: a WAD
can hold `x` and `x/y` at the same time, and no file system holds both.

The extractor resolves the clash by renaming `x` to `x.ltk`. Which of the pair moves is worked out
over the extraction's own paths **before it writes any of them**, so one archive and one hash table
give one output tree on every run and on every host.

This document records the measurements behind that choice, because the question was re-argued
several times from first principles and the answer turns on numbers that are expensive to
re-derive.

## 2. The phenomenon is real, new, and small

Measured against real CDTB hash tables:

| Table                                | Paths     | Extensionless | Also a directory of another path |
| ------------------------------------ | --------- | ------------- | -------------------------------- |
| `hashes.game.txt` (March 2024)       | 1,117,870 | 866 (0.08%)   | **88**                           |
| `hashes.game.txt` (January 2023)     | 814,750   | 0             | **0**                            |
| `hashes.lcu.txt`                     | 83,355    | 0             | 0                                |

Four things follow, and each one settled an argument:

- **All 88 clashes are extensionless.** They are Riot's `clientstates/...` UI definitions, such as
  `clientstates/common/ux/messageboxdialog` next to
  `clientstates/common/ux/messageboxdialog/uibase`.
- **The phenomenon is new and growing.** Zero in 2023, 866 in 2024. It is not a historical quirk to
  be waited out.
- **Extensionless-ness is a correlate, not the cause.** Every clash so far is between extensionless
  paths, but nothing stops Riot shipping `a.bin` next to `a.bin/c`. A rule keyed on the extension
  would rename 866 paths to catch 88, and would still miss the case it was not keyed on.
- **`.ltk` is an unambiguous marker.** No path in any table has a component named `ltk` or ending in
  `.ltk`.

One thing is inferred rather than measured: whether both halves of a clashing pair land in the
*same* WAD. The only local archives are 8.18-era, which predate these paths, and all scanned clean.
A modern `Global` or UI WAD would settle it.

## 3. The options

| #   | Policy                                                       | Deterministic | Exact     | Cost                                   |
| --- | ------------------------------------------------------------ | ------------- | --------- | -------------------------------------- |
| 1   | Resolve at write time                                        | No            | Yes       | None                                   |
| 2   | **Deterministic prefix pass**                                | Yes           | Yes       | One extra resolve per chunk, plus a set. Measured below |
| 3   | Suffix every extensionless path                              | Yes           | Heuristic | 866 renames to catch 88, still incomplete |
| 4   | Universal `.ltk` on every file                               | Yes           | Yes       | 1.1M files pay for 88                  |
| 5   | `ExtractLayout::Flat`                                        | Yes           | Yes       | Loses the hierarchy                    |
| 6   | Sidecar manifest                                             | Yes           | Yes       | Belongs at a project-format layer, not here |
| 7   | Rename the directory instead                                 | —             | —         | Cascades to every child                |

**Option 2 was chosen**, with option 1 kept as the backstop.

Option 1 is what 0.4.0 shipped, and it is order-dependent in a way that is worse than it first
looks. The two branches are not equivalent:

```
x   written first -> x/y fails       -> x/y becomes <hash>.<ext> at the output root, path lost
x/y written first -> x is a directory -> x becomes x.ltk,        path recoverable
```

So the same archive and the same table could produce either a tree that keeps both paths or a tree
that threw one away, depending on chunk order and worker scheduling. Making the choice up front
picks the good branch every time.

## 4. The marker is appended, never substituted

`foo.bin` becomes `foo.bin.ltk`, not `foo.ltk.bin` and not `foo.ltk`. Stripping a trailing `.ltk`
gives the original name back exactly, which is what a caller hashing an extracted file's path back
to its chunk needs.

The earlier `<stem>.ltk.<ext>` scheme was lossy and is gone. It computed the wrong chunk hash for
extensionless paths, silently:

```
before: assets/foo -> assets/foo.ltk.dds -> inverse gives assets/foo.dds -> wrong hash (2 of 5 cases)
after:  assets/foo -> assets/foo.ltk     -> inverse gives assets/foo     -> right hash (0 of 5 wrong)
```

**The known cost of appending:** a tool that picks files by extension will not find a renamed one.
`assets/thing.bin` extracted as `assets/thing.bin.ltk` is invisible to a `*.bin` glob. Inserting the
marker before the extension (`assets/thing.ltk.bin`) would avoid that, at the price of an inverse
that misreads any genuine path shaped `a.ltk.b` — a shape the CDTB scan did not rule out, having
checked only for paths *ending* in `.ltk`.

Appending was kept, because the cost is empirically zero: every measured clash is between paths with
no extension, so a renamed file has no extension to lose. The trade would buy nothing real and
would give up an exact inverse. If a clash between paths that carry extensions ever shows up in a
real table, this is the decision to revisit.

## 5. What the extractor does

1. Before any write, resolve every chunk of the extraction once and collect the set of directories
   those paths name — every prefix of every path, excluding the paths themselves.
2. A path in that set is renamed to `<name>.ltk`. Anything else keeps its name.
3. A directory the output tree held *already*, from an earlier extraction or from anything else, is
   still only found by the write failing, so option 1 remains as the backstop for it, along with
   `InvalidFilename` and the Windows long-path case.

Both the prefix set and the backstop report the move under `ExtractReport::displaced`, with
`PathIssue::Refused` and the file the chunk actually landed in.

The pass costs little, because directories dedupe hard. Measured on 8.18 archives against the
1.1M-entry March 2024 table:

| Archive              | Chunks | Directories | Pass  | Set     | Whole extraction |
| -------------------- | ------ | ----------- | ----- | ------- | ---------------- |
| `DATA.wad.client`    | 22,929 | 2,767       | 6.4ms | 0.18 MB | 4.79s, 1.36 GB   |
| `Global.wad.client`  | 3,763  | 410         | 1.3ms | 0.03 MB | 0.72s, 134 MB    |

That is 0.13% of the extraction it makes deterministic. Both archives displaced nothing, which is
the expected result for 8.18 content: these paths postdate it.

Details that matter:

- **Separators are normalised.** `/` and `\` both separate, and empty components and `.` are
  dropped, so a table written on Windows and one written anywhere else name the same directories.
  The check is deliberately not `Utf8Path`-based: `Utf8Path` treats `\` as a separator on Windows
  and as an ordinary character elsewhere, which would make the output tree host-dependent.
- **Refused and filtered paths make no directory.** A path `is_evil` rejects, or one the path filter
  drops, is never written, so it cannot force a rename on anything.
- **The type filter cannot be applied up front**, because a chunk's kind is not known until its
  bytes are decompressed. A chunk that filter later drops still counts as a directory of the path it
  names, so a type-filtered extraction can rename a path that did not strictly need it. The move is
  reported, and the combination is vanishingly rare.
- **The suffix goes on the path string, not through `set_file_name`.** `set_file_name` re-joins the
  path with the host separator, which would report `assets\thing.ltk` on Windows where every
  un-renamed chunk reports `assets/thing`. Stripping the suffix off that gives a path the archive
  was never built from, so the round trip in section 4 would fail on exactly the files it exists
  for. Appending to the whole path is the same operation, because a path's file name is its tail.
- **A directory over the suffixed name sends the chunk to its hash.** A table can name `x`, `x/y`
  and `x.ltk/z` together. Nothing is left to suffix onto, since a second `.ltk` would no longer
  strip back to `x`, so the chunk takes `<hash>.<ext>`. That is decided in the pre-pass and not by
  letting the write fail, because failing the write is the order-dependent branch this design
  exists to remove.
- **The hex name a nameless chunk takes is checked too.** A hash table naming `<hash>/y` for the very
  hash a nameless chunk lands under used to abort the entire extraction: the write failed, and the
  fallback name a refused write reaches for is the same hex name that just failed. It now takes the
  suffix like any other name, and `is_hex_chunk_path` still recognises the result.

## 6. Filesystem behaviour this relies on

Probed with real binaries on Windows and Unix, and worth not re-deriving:

| Condition                     | Windows            | Unix            |
| ----------------------------- | ------------------ | --------------- |
| Directory where the file goes | `PermissionDenied` | `IsADirectory`  |
| File where the directory goes | `AlreadyExists`    | `NotADirectory` |

`PermissionDenied` is ambiguous on Windows — a genuinely unopenable file reports it too — so
`is_path_conflict` breaks the tie with `path.is_dir()`.

Two more, both feeding `is_evil`:

- `notes.txt.` and `notes.txt` are the **same file** on Windows, which strips the trailing dot before
  it looks the name up. A path ending in a dot or a space is refused, because it would otherwise
  walk straight past the check for two chunks claiming one path.
- Rust reaches the file system through a `\\?\` verbatim path, which does **not** resolve device
  names. `NUL.bin`, `CON`, `COM1.txt` and `aux` are all ordinary files. A device-name blocklist was
  written, measured to be useless, and deleted. Do not re-add it.
- `sub/../../x.txt` does silently escape the output directory, so `..` is refused. `...` and `.. `
  are *not* traversals and fail with `NotFound`.

## 7. What this does not cover

Containment is lexical. `is_evil` says the joined path cannot name a file outside the output
directory; it says nothing about a symlink an output tree already holds. If an extraction is ever
run into a directory an attacker can pre-seed, `cap-std` and OS-enforced containment through
`openat` is the real answer. That is separate, larger work.
