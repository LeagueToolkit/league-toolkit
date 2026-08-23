use std::{fs::File, path::PathBuf, str::FromStr};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some((input_path, output_path)) = args
        .next()
        .and_then(|p| PathBuf::from_str(&p).ok())
        .zip(args.next().and_then(|p| PathBuf::from_str(&p).ok()))
    else {
        eprintln!("Usage: './rito_to_bin [PATH_TO_RITOBIN] [OUTPUT_BIN_PATH]'");
        return;
    };
    println!("Converting {input_path:?} to bin...");

    let text = std::fs::read_to_string(input_path).unwrap();

    let cst = ltk_ritobin::Cst::parse(&text);
    if !cst.errors.is_empty() {
        eprintln!("Errors while parsing:");
        for err in &cst.errors {
            eprintln!("- {err:#?}");
        }
        return;
    }

    let bin = match cst.build_bin(&text).finish() {
        Ok(bin) => bin,
        Err(partial) => {
            eprintln!("Errors while converting to bin:");
            for diag in &partial.diagnostics {
                eprintln!("- {diag:#?}");
            }
            return;
        }
    };

    let mut file = File::create(output_path).unwrap();
    bin.to_writer(&mut file).unwrap();
}
