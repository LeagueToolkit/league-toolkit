use ltk_ritobin::{
    span::{DebugSpans, SpannedToString as _},
    Span, Spanned,
};

#[derive(Debug, DebugSpans)]
struct Entry {
    key: Span,
    count: Spanned<u32>,
}

#[test]
fn derive_resolves_span_fields_in_a_dependent_crate() {
    let entry = Entry {
        key: Span::new(0, 3),
        count: Spanned::new(Span::new(6, 7), 2),
    };
    assert_eq!(
        entry.spanned_to_string("foo = 2"),
        "Entry { key: `foo`, count: Spanned { span: `2`, value: 2 } }"
    );
}
