use std::io::{BufRead, BufReader};

use ltk_ritobin::validate::error::MultiBinError;

fn main() -> miette::Result<()> {
    let input_path = std::env::args().nth(1).unwrap();
    println!("Parsing {input_path:?}...");

    let input = BufReader::new(std::fs::File::open(input_path).unwrap())
        .lines()
        .map_while(Result::ok)
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");

    let (_, statements) = ltk_ritobin::parse(&input).unwrap();
    ltk_ritobin::validate(statements).map_err(|errs| MultiBinError {
        source_code: input.to_string(),
        related: errs,
    })?;
    Ok(())
}
