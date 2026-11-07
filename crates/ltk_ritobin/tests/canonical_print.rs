use std::io::Cursor;

use ltk_meta::{path::PropertyPathErrorKind, BinFile, Error as MetaError};
use ltk_ritobin::{ast::diagnostics::Diagnostic, print::PrintConfig, Cst, Print};

fn fixture(name: &str) -> (Vec<u8>, String) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/");
    let bin = std::fs::read(format!("{dir}{name}.bin")).unwrap();
    let text = std::fs::read_to_string(format!("{dir}{name}.rito")).unwrap();
    (bin, text)
}

fn assert_canonical_print(name: &str) {
    let (bin, text) = fixture(name);
    let file = BinFile::from_reader(&mut Cursor::new(&bin)).unwrap();
    let printed = file.print_with_config(PrintConfig::canonical()).unwrap();
    pretty_assertions::assert_str_eq!(
        printed,
        text,
        "canonical print of {name}.bin == {name}.rito"
    );
}

#[test]
fn canonical_prop() {
    assert_canonical_print("prop_small");
}
#[test]
fn canonical_prop_large() {
    assert_canonical_print("prop_large");
}
#[test]
fn canonical_prop_v1() {
    assert_canonical_print("edge_v1");
}
#[test]
fn canonical_ptch() {
    assert_canonical_print("ptch");
}

#[test]
fn edge_ptch() {
    let (bin, text) = fixture("edge_ptch");

    let err = BinFile::from_reader(&mut Cursor::new(&bin)).unwrap_err();
    assert!(
        matches!(err, MetaError::OverrideDependencies(1)),
        "a patch binary with a dependency is rejected, got {err:?}"
    );

    let cst = Cst::parse(&text);
    assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);

    let (file, diagnostics) = cst.build(&text);
    assert!(file.is_override(), "a PTCH file builds a BinFile::Override");

    let kinds: Vec<&Diagnostic> = diagnostics.iter().map(|d| &d.diagnostic).collect();
    assert_eq!(kinds.len(), 2, "two forbidden records, got {kinds:#?}");
    assert!(
        kinds
            .iter()
            .any(|d| matches!(d, Diagnostic::PatchLinked { .. })),
        "the non-empty linked root is named: {kinds:#?}"
    );
    assert!(
        kinds.iter().any(|d| matches!(
            d,
            Diagnostic::InvalidPropertyPath { error, .. }
                if error.kind() == PropertyPathErrorKind::EmptySegment
        )),
        "the empty path is named: {kinds:#?}"
    );
}
