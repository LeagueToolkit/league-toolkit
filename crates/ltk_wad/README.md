# ltk_wad

Reading, extracting and building League of Legends WAD archives, the `.wad.client` files that hold the assets of the game.

- A WAD is a table of chunks followed by their bytes.
- Each chunk is one file, compressed on its own with gzip or zstd, or stored as is.
- The archive keeps no file names. The key of a chunk is the xxh64 of its lower-case path, typed as [`WadHash`](https://docs.rs/ltk_hash).
- A name comes back only through a hash table, such as the one [CommunityDragon](https://github.com/CommunityDragon/Data) maintains, or out of the `.bin` files inside the archive.

The crate supports reading all versions and writing the latest one, and is part of [League Toolkit](https://github.com/LeagueToolkit/league-toolkit). The umbrella crate exposes it as `league_toolkit::wad` behind the `wad` feature.

```toml
[dependencies]
ltk_wad = "0.4"
ltk_hash = "0.4" # to hash paths yourself
```

## Contents

- [Feature flags](#feature-flags)
- [Reading an archive](#reading-an-archive)
- [Naming chunks](#naming-chunks)
- [Extracting to disk](#extracting-to-disk)
- [Recovering names from the bins](#recovering-names-from-the-bins)
- [Building an archive](#building-an-archive)
- [Signatures](#signatures)
- [Parallel pipelines](#parallel-pipelines)

## Feature flags

| Feature        | Default | What it does                                                                          |
| -------------- | ------- | ------------------------------------------------------------------------------------- |
| `zstd`         | yes     | zstd through the `zstd` crate, which builds the C library                             |
| `ruzstd`       | no      | zstd in pure Rust, for targets with no C toolchain. Slower, and exclusive with `zstd` |
| `rust_backend` | no      | `ruzstd` plus the pure Rust backend of `flate2`, so nothing in the crate compiles C   |
| `serde`        | no      | `Serialize` and `Deserialize` for `Wad`, its chunk table and each chunk               |

## Reading an archive

```rust
use std::fs::File;
use ltk_hash::Hash as _;
use ltk_wad::{Wad, WadHash};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wad = Wad::mount(File::open("Aatrox.wad.client")?)?;
    println!("{} chunks", wad.chunks().len());

    // The table is sorted by hash, and a chunk is a few numbers.
    for chunk in wad.chunks() {
        println!(
            "{:016x} {:?} {} bytes",
            chunk.path_hash, chunk.compression_type, chunk.uncompressed_size
        );
    }

    // A lookup takes the hash of the path, and the bytes come back decompressed.
    let hash = WadHash::hash_str("data/characters/aatrox/aatrox.bin");
    if let Some(chunk) = wad.chunks().get(hash).copied() {
        let bytes = wad.load_chunk_decompressed(&chunk)?;
        println!("{} bytes of bin", bytes.len());
    }
    Ok(())
}
```

- `Wad::mount` reads the header and the chunk table, and keeps the source open. Any `Read + Seek` is a source.
- The bytes of a chunk come off the source when you ask for them.
- `load_chunk_raw` gives the compressed bytes. `decompress_raw` or a `ChunkDecoder` turns them into the file.
- `Wad` keeps one decoder, so a run of `load_chunk_decompressed` calls pays for one zstd context.

On `WadHash`:

- `WadHash::hash_str` is a method of the `ltk_hash::Hash` trait, which is why the example imports it.
- A hash you already hold is `WadHash(value)`.
- `WadHash` parses from its sixteen hex digits with `str::parse`, and prints as hex.

## Naming chunks

Whatever names the chunks is a `PathResolver`: `resolve(hash)` answers the path, or `None` for a hash it has no name for. These are resolvers:

- every `HashMap<WadHash, String>`
- a reference, a `Box` or an `Arc` of any resolver
- `NoResolver`, which names nothing
- `RecoveredNames`, the names read out of the bins, and `over` layers it on top of another resolver

A CommunityDragon hash table is one name per line, as the hash in hex, a space, and the path:

```rust
use std::collections::HashMap;
use ltk_wad::WadHash;

let names: HashMap<WadHash, String> = std::fs::read_to_string("hashes.game.txt")?
    .lines()
    .filter_map(|line| line.split_once(' '))
    .filter_map(|(hash, path)| Some((hash.parse().ok()?, path.to_owned())))
    .collect();
```

## Extracting to disk

```rust
use ltk_wad::{ExistingFilePolicy, WadExtractor};

let mut extractor = WadExtractor::new(&names)
    .with_filter(|path| path.starts_with("assets/characters/"))
    .with_existing_file_policy(ExistingFilePolicy::Skip)
    .on_progress(|p| println!("{:5.1}% {}", p.fraction() * 100.0, p.path()));

let report = extractor.extract_all(&mut wad, "out")?;
println!("{report}");
```

What the builder takes:

- `with_filter` keeps the chunks whose path the closure accepts. An unnamed chunk shows as its hash.
- `with_type_filter` keeps only the given `LeagueFileKind`s, by what the bytes identify as.
- `with_layout(ExtractLayout::Flat)` drops the directories. A second chunk of one name takes `name.<hash>.ext`.
- `with_existing_file_policy(ExistingFilePolicy::Skip)` leaves a file that exists, and counts it.
- `on_progress` hears of each chunk once it is done, with the count, the path, the result and the bytes.
- `with_cancel_flag` takes an `AtomicBool`, and the reader tests it before each chunk.
- `with_workers` sets the thread count. The default is the parallelism of the machine, capped at eight.
- `with_name_recovery` reads the bins for the names the resolver lacks. See [below](#recovering-names-from-the-bins).

What it runs:

- `extract_all` extracts every chunk.
- `extract_chunks` takes the hashes of the chunks to extract, and lists the ones the archive lacks under `report.missing`.

How it runs:

- The reader walks the archive in order on the calling thread.
- It hands each chunk to a worker over a channel bounded by the worker count, and the worker decompresses and writes it.
- The resolver, the path filter and the progress callback stay on the calling thread, so none of them needs to be `Sync`.

Where a chunk lands on disk:

| The chunk                                              | Lands at                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------ |
| The resolver names it                                  | Its path, with every directory of it                                     |
| Nothing names it                                       | Its hash as sixteen hex digits, with the extension its bytes identify as |
| Its name has no extension, or is an existing directory | `<stem>.ltk.<ext>`, or `<stem>.ltk` when the bytes identify as nothing   |
| Its name is too long for the file system               | `<hash>.<ext>` in the output directory itself                            |

What comes back:

- An `ExtractReport` counts the chunks written, skipped and missing, the bytes, and the chunks by kind. It prints as one line.
- A chunk that fails to read, decompress or write fails the extraction with `WadError::Chunk`, which wraps the cause with the hash and the path of the chunk.

## Recovering names from the bins

The `.bin` files of League name the assets they use by path. So the names of many chunks a hash table lacks sit inside the archive itself. `with_name_recovery()` reads them out before the extraction writes anything:

```rust
let report = WadExtractor::new(&names)
    .with_name_recovery()
    .extract_all(&mut wad, "out")?;
println!(
    "{} names from {} bins",
    report.recovered.len(),
    report.recovered.bins_scanned
);
```

A match is exact: a string counts only when its hash is the hash of a chunk the resolver could not name. The pass stays cheap:

- Nothing runs when the resolver names every chunk.
- The scan finds a bin by its name, or by its first bytes, which it decodes from the first compressed block alone.
- It reads the strings out as the length-prefixed runs the format writes, with no parse of the object tree.
- The bins decompress on the worker threads.

On an archive of 30,000 chunks the pass takes tens of milliseconds with a hash table, and under a second with none. `NameRecovery` runs the same scan on its own, and returns the `RecoveredNames`.

## Building an archive

```rust
use std::io::{Cursor, Write as _};
use ltk_wad::{WadBuilder, WadChunkBuilder, WadChunkCompression};

let mut out = Cursor::new(Vec::new());
WadBuilder::default()
    .with_chunk(WadChunkBuilder::default().with_path("data/characters/aatrox/aatrox.bin"))
    .with_chunk(
        WadChunkBuilder::default()
            .with_path("assets/characters/aatrox/skins/base/aatrox_base_tx_cm.dds")
            .with_force_compression(WadChunkCompression::None),
    )
    .build_to_writer(&mut out, |path_hash, cursor| {
        // The bytes of the chunk at `path_hash`, from wherever they live.
        cursor.write_all(&bytes_for(path_hash))?;
        Ok(())
    })?;
```

- The builder writes the chunks in hash order, which the game requires.
- It picks the compression of each chunk by what its bytes identify as, unless `with_force_compression` says otherwise.
- `with_path` hashes a path for the chunk, and `with_hash` takes a hash you already hold.
- `with_prebuilt_signature` and `with_prebuilt_checksum` keep the header of a mounted archive, for a byte-identical rebuild.

## Signatures

Riot signs the chunk table. `verify_signature` checks the embedded PKCS#1 v1.5 signature over the SHA-256 of the table, and `RITO_PKEY` is the public key of Riot:

```rust
use ltk_wad::rsa::{pkcs8::DecodePublicKey as _, RsaPublicKey};
use ltk_wad::RITO_PKEY;

let key = RsaPublicKey::from_public_key_der(RITO_PKEY)?;
let (valid, toc_sha256) = wad.verify_signature(&key)?;
```

## Parallel pipelines

`load_chunk_raw` and `decompress_raw` split the I/O from the CPU work, so a pipeline reads the archive in order and decompresses wherever it likes:

```rust
use ltk_wad::{decompress_raw, WadChunk};

let chunks: Vec<WadChunk> = wad.chunks().iter().copied().collect();
for chunk in &chunks {
    let raw = wad.load_chunk_raw(chunk)?;
    // This can run on another thread. `wad` is not borrowed.
    let bytes = decompress_raw(&raw, chunk.compression_type, chunk.uncompressed_size)?;
}
```

Three ways to decode:

- `decompress_raw` decodes one chunk, and builds a zstd context for it.
- `ChunkDecoder` keeps one zstd context between chunks, the right tool for a thread that decodes many.
- `decompress_prefix`, on either, decodes the first bytes of a chunk alone from a prefix of its raw bytes, enough to tell its kind.

## License

MIT OR Apache-2.0, at your option.
