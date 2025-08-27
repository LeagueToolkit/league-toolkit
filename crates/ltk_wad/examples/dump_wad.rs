use std::{
    env,
    error::Error,
    fs::File,
    io::{stderr, stdout, Read, Write},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use binrw::BinRead as _;
use itertools::Itertools;
use ltk_wad::Wad;
use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

use memmap::Mmap;

fn main() -> Result<(), Box<dyn Error>> {
    let file: PathBuf = env::args()
        .nth(1)
        .expect("Missing file path argument!")
        .parse()
        .expect("Invalid file path");
    println!("-- {file:?} --");

    let file = File::open(file).unwrap();

    let mmap = unsafe { Mmap::map(&file).unwrap() };

    let wad: Wad<_> = Wad::mount(Arc::new(mmap)).unwrap();
    println!("v{}.{}", wad.version().0, wad.version().1);

    {
        let entry = wad.entries.first_key_value().unwrap().1;
        println!("{entry:#?}");
    }

    let (wad, entries) = wad.explode();
    let entry = entries.first_key_value().unwrap().1;
    println!("{entry:#?}");

    let mut total = 0;

    let now = Instant::now();
    let mut stderr = stderr().lock();

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
