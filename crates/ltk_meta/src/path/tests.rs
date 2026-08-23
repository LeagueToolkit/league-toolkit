//! Tests for the property path grammar.

use std::borrow::Cow;

use super::{
    KeyLiteral, PropertyPath, PropertyPathError as Error, PropertyPathErrorKind as Kind, Segment,
    Subscript,
};

fn parse(path: &str) -> PropertyPath {
    PropertyPath::new(path).unwrap_or_else(|e| panic!("{path:?} should parse, got {e}"))
}

fn error(path: &str) -> Error {
    PropertyPath::new(path).expect_err(&format!("{path:?} should not parse"))
}

fn field(name: &str) -> Segment<'_> {
    Segment {
        name,
        subscript: None,
    }
}

fn index(name: &str, index: u32) -> Segment<'_> {
    Segment {
        name,
        subscript: Some(Subscript::Index(index)),
    }
}

fn key<'a>(name: &'a str, key: KeyLiteral<'a>) -> Segment<'a> {
    Segment {
        name,
        subscript: Some(Subscript::Key(key)),
    }
}

#[test]
fn parses_fields() {
    let path = parse("Position.UIRect.Size");
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [field("Position"), field("UIRect"), field("Size")]
    );
}

#[test]
fn parses_a_single_field() {
    let path = parse("Enabled");
    assert_eq!(path.segments().collect::<Vec<_>>(), [field("Enabled")]);
}

#[test]
fn parses_indices() {
    let path = parse("Elements[3]");
    assert_eq!(path.segments().collect::<Vec<_>>(), [index("Elements", 3)]);

    // strtol with base 0: hex and octal are accepted, and the text is preserved.
    let path = parse("AnimationItems[0x1].SpeedScale");
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [index("AnimationItems", 1), field("SpeedScale")]
    );
    assert_eq!(path.as_str(), "AnimationItems[0x1].SpeedScale");

    assert_eq!(
        parse("A[0X1f]").segments().collect::<Vec<_>>(),
        [index("A", 31)]
    );
    assert_eq!(
        parse("A[017]").segments().collect::<Vec<_>>(),
        [index("A", 15)]
    );
    assert_eq!(
        parse("A[0]").segments().collect::<Vec<_>>(),
        [index("A", 0)]
    );
    assert_eq!(
        parse("A[4294967295]").segments().collect::<Vec<_>>(),
        [index("A", u32::MAX)]
    );
}

#[test]
fn parses_keys() {
    let path = parse(r#"PerAttachmentMaterial{"weapon"}"#);
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key("PerAttachmentMaterial", KeyLiteral::from("weapon"))]
    );

    // JSON whitespace is allowed around the key and kept in the text.
    let path = parse("Lookup{ 12 }");
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key("Lookup", KeyLiteral::Number("12"))]
    );
    assert_eq!(path.as_str(), "Lookup{ 12 }");

    assert_eq!(
        parse("A{true}.B").segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::Bool(true)), field("B")]
    );
    assert_eq!(
        parse("A{false}").segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::Bool(false))]
    );
    assert_eq!(
        parse("A{-1.5e+3}").segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::Number("-1.5e+3"))]
    );
}

#[test]
fn unescapes_string_keys() {
    let path = parse(r#"A{"a\"b\\cA\n"}"#);
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key(
            "A",
            KeyLiteral::String(Cow::Owned("a\"b\\cA\n".into()))
        )]
    );

    // A surrogate pair, as rapidjson would decode it.
    let path = parse(r#"A{"😀"}"#);
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::String(Cow::Owned("\u{1f600}".into())))]
    );

    // The closing brace of the subscript is not the one inside the string.
    let path = parse(r#"A{"}"}.B"#);
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::from("}")), field("B")]
    );
}

#[test]
fn hashes_names_case_insensitively() {
    let lower = parse("size");
    let upper = parse("SIZE");

    assert_ne!(lower, upper);
    assert_eq!(
        lower.segments().next().unwrap().name_hash(),
        upper.segments().next().unwrap().name_hash()
    );
    assert_eq!(
        parse("Position").segments().next().unwrap().name_hash(),
        ltk_hash::BinHash(0x934f_4e0a)
    );
}

#[test]
fn rejects_empty_segments() {
    for path in ["", "Position.", ".Position", "A..B"] {
        assert_eq!(error(path).kind(), Kind::EmptySegment, "{path:?}");
    }
    assert_eq!(error("Position.").offset(), 9);
    assert_eq!(error(".Position").offset(), 0);
}

#[test]
fn rejects_trailing_characters() {
    let e = error("Elements[3]x");
    assert_eq!(e.kind(), Kind::UnexpectedCharacter('x'));
    assert_eq!(e.offset(), 11);

    assert_eq!(error("A]B").kind(), Kind::UnexpectedCharacter(']'));
    assert_eq!(error("A}B").kind(), Kind::UnexpectedCharacter('}'));
    assert_eq!(error("A(B)").kind(), Kind::UnexpectedCharacter('('));
}

#[test]
fn rejects_a_second_subscript() {
    // The format has no nested containers, so a segment carries at most one subscript.
    let e = error("A[1][2]");
    assert_eq!(e.kind(), Kind::DoubleSubscript);
    assert_eq!(e.offset(), 4);
    assert_eq!(error("A[1]{2}").kind(), Kind::DoubleSubscript);
}

#[test]
fn rejects_unbalanced_brackets() {
    for path in ["A[(1]", "A[1", "A{1", r#"A{"1"#] {
        let kind = error(path).kind();
        assert!(
            matches!(kind, Kind::UnbalancedBracket | Kind::InvalidIndex),
            "{path:?} gave {kind}"
        );
    }
    assert_eq!(error("A[1").kind(), Kind::UnbalancedBracket);
    assert_eq!(error("A{1").kind(), Kind::UnbalancedBracket);
}

#[test]
fn rejects_bad_indices() {
    for path in [
        "A[-1]", "A[ 3 ]", "A[3.0]", "A[]", "A[0x]", "A[08]", "A[+1]",
    ] {
        assert_eq!(error(path).kind(), Kind::InvalidIndex, "{path:?}");
    }
    // One past a u32.
    assert_eq!(error("A[4294967296]").kind(), Kind::InvalidIndex);
}

#[test]
fn rejects_bad_keys() {
    for path in [
        "A{null}", "A{[1]}", "A{}", "A{tru}", "A{1 2}", "A{01}", "A{-}",
    ] {
        assert_eq!(error(path).kind(), Kind::InvalidKey, "{path:?}");
    }
}

#[test]
fn rejects_paths_that_do_not_fit_the_wire_format() {
    let long = "A".repeat(PropertyPath::MAX_LEN + 1);
    let e = error(&long);

    assert_eq!(e.kind(), Kind::TooLong(PropertyPath::MAX_LEN + 1));
    assert!(PropertyPath::new("A".repeat(PropertyPath::MAX_LEN)).is_ok());
}

#[test]
fn pushes_pieces() {
    let mut path = parse("Position");
    path.push_field("Elements").unwrap();
    path.push_index(3).unwrap();
    path.push_field("Lookup").unwrap();
    path.push_key(&KeyLiteral::from("weapon")).unwrap();

    assert_eq!(path.as_str(), r#"Position.Elements[3].Lookup{"weapon"}"#);

    // A second subscript is rejected and leaves the path alone.
    let before = path.clone();
    assert_eq!(
        path.push_index(1).unwrap_err().kind(),
        Kind::DoubleSubscript
    );
    assert_eq!(path, before);

    assert_eq!(
        path.push_field("A.B").unwrap_err().kind(),
        Kind::UnexpectedCharacter('.')
    );
    assert_eq!(path.push_field("").unwrap_err().kind(), Kind::EmptySegment);
    assert_eq!(path, before);
}

#[test]
fn pushes_escaped_keys() {
    let mut path = parse("A");
    path.push_key(&KeyLiteral::String(Cow::Borrowed("a\"b\n")))
        .unwrap();

    assert_eq!(path.as_str(), r#"A{"a\"b\n"}"#);
    assert_eq!(
        path.segments().collect::<Vec<_>>(),
        [key("A", KeyLiteral::String(Cow::Owned("a\"b\n".into())))]
    );
}

#[test]
fn round_trips_through_text() {
    for text in [
        "Enabled",
        "Position.UIRect.Size",
        "Elements[3]",
        "AnimationItems[0x1].SpeedScale",
        r#"PerAttachmentMaterial{"weapon"}"#,
        "Lookup{ 12 }",
    ] {
        let path: PropertyPath = text.parse().unwrap();
        assert_eq!(path.to_string(), text);
        assert_eq!(PropertyPath::try_from(text).unwrap(), path);

        // Segments print in canonical form: the same text, with indices in decimal and
        // no whitespace around a key.
        let printed = path
            .segments()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>()
            .join(".");
        assert_eq!(
            printed,
            text.replace("[0x1]", "[1]").replace("{ 12 }", "{12}")
        );
    }
}
