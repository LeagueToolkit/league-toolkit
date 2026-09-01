# The WAD extractor

Spec and design record for `ltk_wad::extractor`: the public surface, the routines behind it, and
the measurements behind the naming rules.

Status: implemented, 2026-08-26. Absorbs the path-clash design record of 2026-08-25, which
superseded the write-time-only resolution that shipped in `ltk_wad` 0.4.0.

## <a id="s1"></a>1. What it is

A WAD is a flat map from a path hash to bytes. A file system is a tree, in which every name is a
file **exclusive-or** a directory. The extractor projects the one onto the other, and everything
interesting about it comes from the ways that projection is imperfect:

- The archive stores hashes, not paths, so a **resolver** has to supply the names, and a chunk
  nothing names needs a fallback ([section 3](#s3)).
- The projection is not injective: a WAD can hold `x` and `x/y` at the same time, and no file
  system holds both, so one of the pair has to **move** ([section 6](#s6)).
- The resolver's paths are **untrusted**, so some of them must never reach the file system
  ([section 8](#s8)).
- Extraction is I/O- and decompression-bound, so the work is **parallel**, and none of the above
  may depend on which worker gets where first ([section 5](#s5)).

The guarantee that ties these together: **one archive and one hash table give one output tree, on
every run and on every host.** Every naming decision is either made before the first write or
shown to be order-independent; the exceptions are named in [section 6](#s6).

## <a id="s2"></a>2. Public surface: `WadExtractor`

`WadExtractor` is configuration plus execution. Build one with `new(&resolver)`, configure it with
`with_*` methods, run it with `extract_all` or `extract_chunks`. One extractor runs any number of
extractions.

| Method                      | Effect                                                       | Default                              |
| --------------------------- | ------------------------------------------------------------ | ------------------------------------ |
| `new(&dyn PathResolver)`    | Names chunks through the resolver                            | required                             |
| `with_filter(fn)`           | Keep only chunks whose path the filter accepts               | keep all                             |
| `with_type_filter(kinds)`   | Keep only chunks whose bytes identify as one of `kinds`      | keep all                             |
| `on_progress(fn)`           | Hear of each chunk once it is done, skipped chunks included  | silent                               |
| `with_layout(layout)`       | `ExtractLayout::Paths` or `::Flat`                           | `Paths`                              |
| `with_existing_file_policy` | `ExistingFilePolicy::Overwrite` or `::Skip`                  | `Overwrite`                          |
| `with_cancel_flag(&flag)`   | Stop early once the `AtomicBool` reads true                  | run to the end                       |
| `with_workers(n)`           | Threads that decompress and write                            | available parallelism, capped at 8   |
| `with_name_recovery()`      | Read the archive's own bins for names first ([section 4](#s4))       | off                                  |

The two entry points:

- **`extract_all(&mut wad, output_dir)`** extracts every chunk of the archive.
- **`extract_chunks(&mut wad, path_hashes, output_dir)`** extracts a selection, in the order
  given. A hash given twice counts once. A hash the archive holds no chunk for lands under
  `ExtractReport::missing` and is not an error.

Both return `Result<ExtractReport, WadError>`. Both fail on the first chunk the extractor cannot
read, decompress or write, with a `WadError::Chunk` that names it; chunks written before the
failure stay on disk. A pair of the extraction's own clashing paths is **not** a failure: the
extraction moves one of them and finishes ([section 6](#s6)).

The filters differ in when they can run, and it shows. The path filter runs in the up-front pass,
so a path it drops is never written and makes no directory. The type filter cannot run until a
chunk's bytes are decompressed, so a chunk it drops still counts as a directory of the path it
names, and a path that only that chunk clashed with still takes its `.ltk` suffix. The move is
reported, and the combination is vanishingly rare.

## <a id="s3"></a>3. Naming chunks: `PathResolver`

A WAD stores the hash of each chunk's path and not the path, so an extraction needs something to
supply the names:

```rust
pub trait PathResolver {
    fn resolve(&self, path_hash: WadHash) -> Option<String>;
    fn is_known(&self, path_hash: WadHash) -> bool { /* calls resolve */ }
}
```

Every `HashMap<WadHash, String>` is a resolver, and so is a reference, a `Box` or an `Arc` of any
resolver. `NoResolver` names nothing. A chunk no resolver names lands under its hash as sixteen
hex digits, with the extension its bytes identify as, when they identify as anything.
`is_hex_chunk_path` recognises that shape, so a caller can sort a tree extracted earlier into
named and unnamed files; `ExtractProgress::is_named` is the exact answer during the run, since a
real name can also be sixteen hex digits.

The resolver is asked about each chunk exactly once per extraction, in the up-front pass
([section 5](#s5)), and runs on the calling thread, so it does not need to be `Sync`.

## <a id="s4"></a>4. Name recovery

A chunk no external table names can still get its name from the archive itself: the `.bin` files
of a WAD carry path strings. `NameRecovery` scans them without a full parse, telling a bin from
any other chunk by its magic decoded from the first compressed block, then reading the
length-prefixed string runs and keeping the ones that hash to a chunk the archive actually holds.

- Standalone: `NameRecovery::new().run(&mut wad, &resolver)` returns `RecoveredNames`, itself a
  lookup (`get`, `len`, `is_empty`) and layerable over any resolver with
  `recovered.over(&fallback)`, which gives a `LayeredResolver`.
- Inline: `WadExtractor::with_name_recovery()` runs the scan before the extraction writes
  anything, using the same workers and cancel flag. The names land in
  `ExtractReport::recovered`.

Recovered names are resolver paths like any other: untrusted, and checked the same way.

## <a id="s5"></a>5. The pipeline

An extraction is three stages. The first is a pass over metadata; the other two run concurrently.

1. **Resolve (calling thread, before any write).** Every chunk is named through the resolver, here
   and nowhere else. In the same pass, each path is checked against `is_evil` ([section 8](#s8)) and
   the path filter, so what the extraction will refuse and what it will skip is settled first. From
   the surviving paths, `DirectoryPaths` collects every prefix of every path, excluding the paths
   themselves: the set of names the extraction's own output needs as directories. Each name is read
   once and carried from here to the worker that writes the chunk; a worker needs an owned path
   either way, because the job crosses a thread boundary, so nothing is allocated twice.
2. **Read (calling thread).** The reader walks the archive in order and hands each chunk's raw
   bytes to a worker over a channel bounded by the worker count, so memory holds a few chunks
   whatever the archive holds. The cancel flag is checked before each read. The resolver, the
   path filter and the progress callback all stay on this thread, which is what keeps all three
   free of any `Sync` bound; the progress callback hears of each chunk once it is done.
3. **Write (workers).** Each worker decompresses its chunk, applies the type filter, settles the
   final name ([section 6](#s6)), and writes. The workers share a `ChunkWriter`: the directory set
   from stage 1, and a mutex-guarded claim set of names given out so far, which is how a second
   chunk resolving to an already-written path is caught and skipped rather than silently overwriting
   the first. After a failure the workers drain and drop their queues rather than write, so a reader
   blocked on a full channel sees the failure too.

`ExistingFilePolicy::Skip` opens with `create_new`, which makes the existence check and the create
one operation, so two workers can never both write one path and a file that appears between two
chunks is left alone too.

## <a id="s6"></a>6. How a chunk is named on disk

- A path the resolver knows stays as it is.
- A nameless chunk lands under its hash as sixteen hex digits ([section 3](#s3)).
- A name that another path of the same extraction needs as a directory takes a `.ltk` suffix:
  `foo.bin` becomes `foo.bin.ltk`. Which of the pair moves is settled against the stage-1
  directory set, before anything is written. It is always the file, never the directory: renaming
  the directory would cascade to every child under it.
- A name a directory holds where the table also claims the *suffixed* name as a directory (`x`,
  `x/y` and `x.ltk/z` in one table) has nothing left to suffix onto, since a second `.ltk` would
  no longer strip back to `x`. The chunk takes `<hash>.<ext>` in the output directory itself.
  This too is settled in stage 1, not by letting the write fail.
- The hex name of a nameless chunk is checked the same way. A table naming `<hash>/y` for the very
  hash a nameless chunk lands under used to abort the entire extraction, because the fallback
  name a refused write reaches for was the same hex name that just failed. The chunk now takes
  `<hex>.ltk`, and `is_hex_chunk_path` still recognises it.
- A name the file system refuses at write time (a directory left by an *earlier* extraction, an
  invalid name, the Windows long-path limit) becomes `<hash>.<ext>` in the output directory
  itself, losing the directories the path named. This is the one naming decision that only the
  write can make, and it is order-independent: whatever stood in the way was there before the
  extraction started. `is_path_conflict` ([section 9](#s9)) tells a refused name from a failure a
  different name would not mend; the latter ends the run.
- Under `ExtractLayout::Flat` every chunk lands in the output directory by file name alone, and a
  second chunk of one name takes its hash before the extension, as `name.<hash>.ext`. A flat tree
  collides by design, so which chunk is second follows write order; the flat layout is the one
  place the determinism guarantee does not hold, and it gives up the hierarchy the guarantee is
  about.

Nothing else moves a chunk off the name its path gave it. The suffix is added and never
substituted, so stripping a trailing `.ltk` gives the original name back exactly, hex name as much
as path, which is what a caller hashing an extracted file's path back to its chunk needs.

## <a id="s7"></a>7. What comes back

`ExtractProgress` reports one chunk as it finishes: `done`/`total`/`fraction`, `path_hash`,
`path`, `is_named`, `bytes`, an `ExtractResult` saying what became of the chunk (`Extracted`, or
skipped by type, path, existing file, rejected path, duplicate path), and `output_path`, the file
actually written relative to the output directory. The layout, the `.ltk` suffix and a refused
name can each make `output_path` differ from `path`, so a caller that indexes what an extraction
wrote reads the former.

`ExtractReport` sums the run: `extracted`, `skipped_existing`, `skipped_by_filter`, `missing`,
`bytes_written`, `by_kind`, `cancelled`, `recovered`, and `displaced`. A chunk that did not land
at the path its resolver gave is a `DisplacedChunk` under `displaced`, with a `PathIssue` saying
why:

- **`Rejected`**: the path was one the extraction refuses to write ([section 8](#s8)). Nothing was
  written.
- **`Duplicate`**: another chunk claimed the path first. Nothing was written; the first file
  stays. Two hashes resolving to one path means the resolver is wrong about one of them, and an
  extraction that overwrote in silence would lose the difference.
- **`Renamed(path)`**: the chunk was written, at the carried path instead ([section 6](#s6)).

The counts are computed from the list rather than kept alongside it: `report.rejected()`,
`report.duplicates()`, `report.renamed()`. `Display` for the report renders the whole run as one
line.

## <a id="s8"></a>8. Paths the extraction will not write

A resolver's paths are untrusted: a hash table is a third-party download, and name recovery reads
paths out of the archive itself. A path is refused, before anything is written, when joining it
onto the output directory would not give a plain file plainly under that directory:

- it starts at a root, a drive or a network share, so the join ignores the output directory;
- a component is `..`, which reaches the directory above;
- a component holds a `:`, naming a Windows drive or an alternate data stream instead of a file
  the directory lists;
- a component ends in a dot or a space, which Windows strips before it looks the name up, so
  `notes.txt.` and `notes.txt` are one file under two names and would walk past the duplicate
  check;
- or it names no file at all, holding nothing but separators and `.`.

The Windows-only rules apply wherever the extraction runs. That is deliberate: one archive and one
hash table then give one output tree on every host, and a test on any host catches a table that
would misbehave on Windows. It costs two conditions.

The check reads the raw string the resolver gave, with `/` and `\` both separating components
whatever the host. Turning the path into a `Utf8Path` first would normalise away the very things
the check looks for, and would read `\` as a separator on Windows and as an ordinary character
elsewhere, making the output tree host-dependent. The same separator rule builds the directory
set, so a table written on Windows and one written anywhere else name the same directories.

A refused path, like one the path filter drops, is never written and makes no directory, so it
cannot force a rename on anything. A caller's filter is applied after the refusal check, so a
selection cannot mask the fact that its resolver handed out a hostile path.

## <a id="s9"></a>9. The path-clash record

The rename rule in [section 6](#s6) was re-argued several times from first principles, and the
answer turns on numbers that are expensive to re-derive. This section records them.

### <a id="s9.1"></a>9.1 The phenomenon is real, new, and small

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
- **The phenomenon is new and growing.** Zero of either in 2023; 866 and 88 in 2024. It is not a
  historical quirk to be waited out.
- **Extensionless-ness is a correlate, not the cause.** Every clash so far is between extensionless
  paths, but nothing stops Riot shipping `a.bin` next to `a.bin/c`. A rule keyed on the extension
  would rename 866 paths to catch 88, and would still miss the case it was not keyed on.
- **`.ltk` is an unambiguous marker.** No path in any table has a component named `ltk` or ending
  in `.ltk`.

One thing is inferred rather than measured: whether both halves of a clashing pair land in the
*same* WAD. The only local archives are 8.18-era, which predate these paths, and all scanned
clean. A modern `Global` or UI WAD would settle it.

### <a id="s9.2"></a>9.2 The options

| #   | Policy                          | Deterministic | Exact     | Cost                                                      |
| --- | ------------------------------- | ------------- | --------- | --------------------------------------------------------- |
| 1   | Resolve at write time           | No            | Yes       | None                                                      |
| 2   | **Deterministic prefix pass**   | Yes           | Yes       | Holding the names for the run, plus a set. Measured below |
| 3   | Suffix every extensionless path | Yes           | Heuristic | 866 renames to catch 88, still incomplete                 |
| 4   | Universal `.ltk` on every file  | Yes           | Yes       | 1.1M files pay for 88                                     |
| 5   | `ExtractLayout::Flat`           | Yes           | Yes       | Loses the hierarchy                                       |
| 6   | Sidecar manifest                | Yes           | Yes       | Belongs at a project-format layer, not here               |
| 7   | Rename the directory instead    | n/a           | n/a       | Cascades to every child                                   |

**Option 2 was chosen**, with option 1 kept as the backstop for what only the write can see.

Option 1 is what 0.4.0 shipped, and it is order-dependent in a way that is worse than it first
looks. The two branches are not equivalent:

```
x   written first -> x/y fails       -> x/y becomes <hash>.<ext> at the output root, path lost
x/y written first -> x is a directory -> x becomes x.ltk,        path recoverable
```

So the same archive and the same table could produce either a tree that keeps both paths or a
tree that threw one away, depending on chunk order and worker scheduling. Making the choice up
front picks the good branch every time.

### <a id="s9.3"></a>9.3 The marker is appended, never substituted

`foo.bin` becomes `foo.bin.ltk`, not `foo.ltk.bin` and not `foo.ltk`. Stripping a trailing `.ltk`
gives the original name back exactly, which is what a caller hashing an extracted file's path
back to its chunk needs.

The earlier `<stem>.ltk.<ext>` scheme was lossy and is gone. It computed the wrong chunk hash for
extensionless paths, silently:

```
before: assets/foo -> assets/foo.ltk.dds -> inverse gives assets/foo.dds -> wrong hash (2 of 5 cases)
after:  assets/foo -> assets/foo.ltk     -> inverse gives assets/foo     -> right hash (0 of 5 wrong)
```

**The known cost of appending:** a tool that picks files by extension will not find a renamed
one. `assets/thing.bin` extracted as `assets/thing.bin.ltk` is invisible to a `*.bin` glob.
Inserting the marker before the extension (`assets/thing.ltk.bin`) would avoid that, at the price
of an inverse that misreads any genuine path shaped `a.ltk.b`, a shape the CDTB scan did not rule
out, having checked only for paths *ending* in `.ltk`.

Appending was kept, because the cost is empirically zero: every measured clash is between paths
with no extension, so a renamed file has no extension to lose. The trade would buy nothing real
and would give up an exact inverse. If a clash between paths that carry extensions ever shows up
in a real table, this is the decision to revisit.

### <a id="s9.4"></a>9.4 The suffix goes on the path string, not through `set_file_name`

`set_file_name` re-joins the path with the host separator, which would report `assets\thing.ltk`
on Windows where every un-renamed chunk reports `assets/thing`. Stripping the suffix off that
gives a path the archive was never built from, so the round trip above would fail on exactly the
files it exists for. Appending to the whole path is the same operation, because a path's file
name is its tail.

### <a id="s9.5"></a>9.5 What the pass costs

Directories dedupe hard, so the pass is cheap. Measured on 8.18 archives against the 1.1M-entry
March 2024 table:

| Archive             | Chunks | Directories | Pass  | Set     | Whole extraction |
| ------------------- | ------ | ----------- | ----- | ------- | ---------------- |
| `DATA.wad.client`   | 22,929 | 2,767       | 6.4ms | 0.18 MB | 4.79s, 1.36 GB   |
| `Global.wad.client` | 3,763  | 410         | 1.3ms | 0.03 MB | 0.72s, 134 MB    |

That is 0.13% of the extraction it makes deterministic. Both archives displaced nothing, which is
the expected result for 8.18 content: these paths postdate it.

## <a id="s10"></a>10. Filesystem behaviour this relies on

Probed with real binaries on Windows and Unix, and worth not re-deriving:

| Condition                     | Windows            | Unix            |
| ----------------------------- | ------------------ | --------------- |
| Directory where the file goes | `PermissionDenied` | `IsADirectory`  |
| File where the directory goes | `AlreadyExists`    | `NotADirectory` |

`PermissionDenied` is ambiguous on Windows (a genuinely unopenable file reports it too), so
`is_path_conflict` breaks the tie with `path.is_dir()`.

Three more findings, two of them feeding the refusal rules in [section 8](#s8):

- `notes.txt.` and `notes.txt` are the **same file** on Windows, which strips the trailing dot
  before it looks the name up. A path ending in a dot or a space is refused, because it would
  otherwise walk straight past the check for two chunks claiming one path.
- Rust reaches the file system through a `\\?\` verbatim path, which does **not** resolve device
  names. `NUL.bin`, `CON`, `COM1.txt` and `aux` are all ordinary files. A device-name blocklist
  was written, measured to be useless, and deleted. Do not re-add it.
- `sub/../../x.txt` does silently escape the output directory, so `..` is refused. `...` and
  `.. ` are *not* traversals and fail with `NotFound`.

## <a id="s11"></a>11. What this does not cover

Containment is lexical. [Section 8](#s8) says the joined path cannot name a file outside the output
directory; it says nothing about a symlink an output tree already holds. If an extraction is ever
run into a directory an attacker can pre-seed, `cap-std` and OS-enforced containment through
`openat` is the real answer. That is separate, larger work.
