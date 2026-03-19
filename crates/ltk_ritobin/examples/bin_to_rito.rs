use ltk_ritobin::{
    print::{Print, PrintConfig},
    HashMapProvider,
};
use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some((input_path, output_path)) = args
        .next()
        .and_then(|p| PathBuf::from_str(&p).ok())
        .zip(args.next().and_then(|p| PathBuf::from_str(&p).ok()))
    else {
        eprintln!("Usage: './from_bin [PATH_TO_BIN] [OUTPUT_PATH]'");
        return;
    };
    println!("Converting {input_path:?} to ritobin...");

    let mut file = File::open(input_path).map(BufReader::new).unwrap();
    let tree = ltk_meta::Bin::from_reader(&mut file).unwrap();

    println!(" - {} objects", tree.objects.len());

    let mut hashes = HashMapProvider::new();
    hashes.load_from_directory(
        std::env::var("HASH_DIR")
            .ok()
            .and_then(|p| PathBuf::from_str(&p).ok())
            .unwrap_or(std::env::current_dir().unwrap()),
    );
    let text = tree
        .print_with_config(PrintConfig::default().with_hashes(hashes))
        .unwrap();
    std::fs::write(output_path, text).unwrap();
}
