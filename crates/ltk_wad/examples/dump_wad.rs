use std::{
    env,
    error::Error,
    fs::File,
    io::{stdout, Read, Write},
    path::PathBuf,
    time::Instant,
};

use itertools::Itertools;
use ltk_wad::Wad;

fn main() -> Result<(), Box<dyn Error>> {
    let file: PathBuf = env::args()
        .nth(1)
        .expect("Missing file path argument!")
        .parse()
        .expect("Invalid file path");
    println!("-- {file:?} --");

    let file = File::open(file).unwrap();

    let mut wad = Wad::mount(file).unwrap();

    let mut total = 0;

    let mut buf = Vec::new();
    let ids = wad.chunks().keys().copied().sorted().collect::<Vec<_>>();

    let now = Instant::now();
    let mut stdout = stdout().lock();

    for id in ids {
        let mut decoder = wad.chunk_decoder(id)
            .unwrap(/* we know the id exists */)
            .expect("failed to create chunk decoder");

        decoder.read_to_end(&mut buf)?;
        writeln!(
            stdout,
            "{id:x}: {:>10} bytes ({})",
            buf.len(),
            decoder.compression_type(),
        )?;
        total += buf.len();
        buf.clear();
    }
    drop(stdout);

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
