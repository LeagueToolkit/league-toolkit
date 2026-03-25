use std::{path::PathBuf, str::FromStr};

use ltk_ritobin::{
    cst::Cst,
    print::{CstPrinter, PrintConfig, WrapConfig},
};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input_path) = args.next().and_then(|p| PathBuf::from_str(&p).ok()) else {
        eprintln!("Usage: './from_bin [PATH_TO_RITOBIN]'");
        return;
    };

    let size = args
        .next()
        .and_then(|p| usize::from_str(&p).ok())
        .unwrap_or(80);
    eprintln!("Formatting {input_path:?}... (size {size})");

    let input = std::fs::read_to_string(input_path).unwrap();

    let cst = Cst::parse(&input);

    // let mut str = String::new();
    // cst.print(&mut str, 0, &input);
    // eprintln!("#### CST:\n{str}");

    let mut str = String::new();
    CstPrinter::new(
        &input,
        &mut str,
        PrintConfig::default().wrap(WrapConfig::default().line_width(size)),
    )
    .print(&cst)
    .unwrap();

    println!("{str}");
}
