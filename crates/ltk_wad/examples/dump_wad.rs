use std::{
    env,
    error::Error,
    fs::File,
    io::{stderr, stdout, Read, Write},
    path::PathBuf,
    time::Instant,
};

use itertools::Itertools;
use ltk_wad::Wad;
use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

fn main() -> Result<(), Box<dyn Error>> {
    let file: PathBuf = env::args()
        .nth(1)
        .expect("Missing file path argument!")
        .parse()
        .expect("Invalid file path");
    println!("-- {file:?} --");

    let file = File::open(file).unwrap();

    let mut wad = Wad::mount(file).unwrap();
    println!("v{}.{}", wad.version().0, wad.version().1);

    let mut total = 0;

    let mut buf = Vec::new();
    let ids = wad.chunks().keys().copied().sorted().collect::<Vec<_>>();

    let now = Instant::now();
    let mut stderr = stderr().lock();

    for id in ids {
        let mut decoder = wad.chunk_decoder(id)
            .unwrap(/* we know the id exists */)
            .expect("failed to create chunk decoder");

        decoder.read_to_end(&mut buf)?;
        //writeln!(
        //    stderr,
        //    "{id:0>16x}: {:>10} bytes ({})",
        //    buf.len(),
        //    decoder.compression_type(),
        //)?;
        drop(decoder);
        let chunk = wad.chunks().get(&id).unwrap();
        //writeln!(stderr, "expected: {:0>16x}", chunk.checksum())?;
        let checksum = xxh3_64(buf.as_slice());
        //writeln!(stderr, "     got: {:0>16x}", checksum)?;
        total += buf.len();

        // xxh64(input, seed)
        buf.clear();
    }
    drop(stderr);

    let elapsed = now.elapsed();
    println!("==== [SUMMARY] ====");
    println!("{:8.3} MiB", total as f64 / 1_048_576.0);
    println!("{:8.3} sec\n", elapsed.as_secs_f32());
    println!(
        "{:8.3} MiB/sec",
        (total as f64 / elapsed.as_secs_f64()) / 1_048_576.0
    );
    Ok(())
}
