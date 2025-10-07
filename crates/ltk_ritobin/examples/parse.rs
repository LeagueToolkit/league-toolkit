use std::io::{BufRead, BufReader, Read};

fn main() {
    let input_path = std::env::args().nth(1).unwrap();
    println!("Parsing {input_path:?}...");

    let input = BufReader::new(std::fs::File::open(input_path).unwrap())
        .lines()
        .map_while(Result::ok)
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");

    ltk_ritobin::parse(&input).unwrap();
}
