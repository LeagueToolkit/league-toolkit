use ltk_ritobin::{
    print::{CstPrinter, PrintConfig},
    Cst, Print,
};

fn fixture(name: &str) -> (Vec<u8>, String) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/");
    let bin = std::fs::read(format!("{dir}{name}.bin")).unwrap();
    let text = std::fs::read_to_string(format!("{dir}{name}.rito")).unwrap();
    (bin, text)
}

fn assert_cst(text: &str) -> Cst {
    let cst = Cst::parse(text);

    let mut buf = String::new();
    cst.print(&mut buf, text);

    println!("{buf}");

    assert!(cst.errors.is_empty(), "Parse errors: {:#?}", cst.errors);
    cst
}

fn print_equal(text: &str) {
    let config = PrintConfig::default();

    let cst = assert_cst(text);
    let bin = cst.build_bin(text).into_result().unwrap();

    let mut cst_print = String::new();
    CstPrinter::new(text, &mut cst_print, config)
        .print(&cst)
        .unwrap();

    let bin_print = bin.print_with_config(config).unwrap();

    pretty_assertions::assert_str_eq!(cst_print, text, "cst == text");
    pretty_assertions::assert_str_eq!(bin_print, text, "bin == text");
    pretty_assertions::assert_str_eq!(cst_print, bin_print, "cst == bin");
}

#[test]
#[ignore]
fn edge_prop() {
    print_equal(&fixture("edge-prop").1);
}
