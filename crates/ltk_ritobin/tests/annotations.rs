//! Annotations: attachment, queries, argument reading and resolution.
//!
//! The spec is `docs/design/ritobin-annotations.md`.

use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_ritobin::{
    ast::{
        annotation::{
            Annotatable, AnnotationRef, ArgErrorKind, Argument, FromAnnotation, Resolved,
        },
        diagnostics::Diagnostic,
        node::root::RootKind,
        Ast, Value,
    },
    print::CstPrinter,
    Cst, Span, Spanned,
};

const FILE: &str = r#"#PROP_text
@on_type
type: string = "PROP"
version: u32 = 3
linked: list[string] = {
    @on_item
    "DATA/a.bin"
    "DATA/b.bin"
}
@on_entries
entries: map[hash,embed] = {
    @on_root_entry(depth = 2, strict)
    # a comment between the annotation and its target
    "Characters/Test" = SkinCharacterDataProperties {
        @first
        @second
        skinClassification: u32 = 1
        @on_embed
        loadscreen: embed = CensoredImage {
            image: string = "ASSETS/a.tex"
        }
        @on_list
        units: list2[embed] = {
            @on_list_item
            BankUnit {
                name: string = "Test"
            }
        }
        @on_map
        lookup: map[string, u32] = {
            @on_map_entry
            "a" = 1
        }
    }
}
"#;

fn parse(text: &str) -> Ast {
    let cst = Cst::parse(text);
    assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
    cst.build_ast(text)
}

fn names<'a>(ast: &'a Ast, text: &'a str, node: &impl Annotatable) -> Vec<&'a str> {
    ast.annotations
        .with_source(text)
        .on(node)
        .map(|a| a.name())
        .collect()
}

fn entry(ast: &Ast) -> &ltk_ritobin::ast::RootEntry {
    ast.root_entries().next().expect("one root entry")
}

fn property<'a>(ast: &'a Ast, name: &str) -> &'a ltk_ritobin::ast::Property {
    let hash = BinHash::hash_str(name);
    entry(ast)
        .object
        .properties
        .iter()
        .find(|p| p.name.value == hash)
        .unwrap_or_else(|| panic!("no property {name}"))
}

#[test]
fn builds_without_diagnostics() {
    let ast = parse(FILE);
    assert!(
        ast.diagnostics.is_empty(),
        "diagnostics: {:#?}",
        ast.diagnostics
    );
    assert_eq!(ast.annotations.len(), 11);
}

#[test]
fn every_node_kind_is_a_target() {
    let ast = parse(FILE);
    let text = FILE;

    let type_root = ast
        .roots
        .all
        .iter()
        .find(|r| r.name.value == RootKind::Type)
        .unwrap();
    assert_eq!(names(&ast, text, type_root), ["on_type"]);

    let entries_root = ast
        .roots
        .all
        .iter()
        .find(|r| r.name.value == RootKind::Entries)
        .unwrap();
    assert_eq!(names(&ast, text, entries_root), ["on_entries"]);

    let linked = ast
        .roots
        .all
        .iter()
        .find(|r| r.name.value == RootKind::Linked)
        .unwrap();
    let Some(Value::Container { items, .. }) = linked.value.as_ref().and_then(|v| v.as_value())
    else {
        panic!("linked is a list");
    };
    assert_eq!(names(&ast, text, &items[0]), ["on_item"]);
    assert_eq!(names(&ast, text, &items[1]), [] as [&str; 0]);

    assert_eq!(names(&ast, text, entry(&ast)), ["on_root_entry"]);

    assert_eq!(
        names(&ast, text, property(&ast, "skinClassification")),
        ["first", "second"]
    );
    assert_eq!(
        names(&ast, text, property(&ast, "loadscreen")),
        ["on_embed"]
    );
    assert_eq!(names(&ast, text, property(&ast, "units")), ["on_list"]);

    let Some(Value::UnorderedContainer { items, .. }) = property(&ast, "units").value.as_ref()
    else {
        panic!("units is a list2");
    };
    assert_eq!(names(&ast, text, &items[0]), ["on_list_item"]);

    assert_eq!(names(&ast, text, property(&ast, "lookup")), ["on_map"]);
    let Some(Value::Map(map)) = property(&ast, "lookup").value.as_ref() else {
        panic!("lookup is a map");
    };
    assert_eq!(names(&ast, text, &map.entries[0]), ["on_map_entry"]);
}

#[test]
fn target_spans_equal_node_targets() {
    let ast = parse(FILE);
    let targets: Vec<Span> = ast.annotations.iter().map(|a| a.target()).collect();

    let Some(Value::UnorderedContainer { items: units, .. }) =
        property(&ast, "units").value.as_ref()
    else {
        panic!("units is a list2");
    };
    let Some(Value::Map(lookup)) = property(&ast, "lookup").value.as_ref() else {
        panic!("lookup is a map");
    };
    let linked = ast
        .roots
        .all
        .iter()
        .find(|r| r.name.value == RootKind::Linked)
        .unwrap();
    let Some(Value::Container { items: paths, .. }) =
        linked.value.as_ref().and_then(|v| v.as_value())
    else {
        panic!("linked is a list");
    };

    // one per node kind: Root, RootEntry, Property, MapEntry, literal and class list items
    for node in [
        ast.roots.all[0].target(),
        entry(&ast).target(),
        property(&ast, "skinClassification").target(),
        property(&ast, "loadscreen").target(),
        property(&ast, "units").target(),
        property(&ast, "lookup").target(),
        lookup.entries[0].target(),
        units[0].target(),
        paths[0].target(),
    ] {
        assert!(targets.contains(&node), "{node:?} not in {targets:?}");
    }
    let mut sorted = targets.clone();
    sorted.sort_by_key(|s| s.start);
    assert_eq!(targets, sorted, "table is sorted by target start");
}

#[test]
fn applying_to_walks_outward() {
    let ast = parse(FILE);
    let text = FILE;
    let image = &property(&ast, "loadscreen").value;
    let Some(Value::Embedded(object)) = image else {
        panic!("loadscreen is an embed");
    };
    let image = &object.properties[0];

    let chain: Vec<&str> = ast
        .annotations
        .with_source(text)
        .applying_to(image)
        .map(|a| a.annotation().name())
        .collect();
    assert_eq!(chain, ["on_entries", "on_root_entry", "on_embed"]);

    let own: Vec<&str> = ast
        .annotations
        .with_source(text)
        .applying_to(property(&ast, "skinClassification"))
        .map(|a| a.annotation().name())
        .collect();
    assert_eq!(own, ["on_entries", "on_root_entry", "first", "second"]);
}

#[test]
fn a_span_is_its_own_target() {
    let ast = parse(FILE);
    let span = property(&ast, "units").target();
    assert_eq!(names(&ast, FILE, &span), ["on_list"]);
}

#[test]
fn target_at_end_of_file_without_newline() {
    let text = "@a\nversion: u32 = 3";
    let ast = Cst::parse(text).build_ast(text);
    let root = &ast.roots.all[0];
    assert_eq!(names(&ast, text, root), ["a"]);
    assert_eq!(root.target(), Span::new(3, text.len() as u32));
}

#[test]
fn a_comment_is_never_a_target() {
    let text = "@a\n# c\nversion: u32 = 3\n";
    let ast = Cst::parse(text).build_ast(text);
    assert_eq!(names(&ast, text, &ast.roots.all[0]), ["a"]);
}

#[test]
fn trailing_annotations_are_one_diagnostic() {
    for text in [
        "version: u32 = 3\n@a\n@b\n",
        "x: list[u32] = {\n 1\n @a\n @b\n}\n",
    ] {
        let ast = Cst::parse(text).build_ast(text);
        let trailing: Vec<Span> = ast
            .diagnostics
            .iter()
            .filter_map(|d| match d.diagnostic {
                Diagnostic::TrailingAnnotations(span) => Some(span),
                _ => None,
            })
            .collect();
        let a = text.find("@a").unwrap() as u32;
        let b = text.find("@b").unwrap() as u32;
        assert_eq!(trailing, [Span::new(a, b + 2)], "{text:?}");
        assert!(ast.annotations.is_empty());
    }
}

#[test]
fn an_error_tree_keeps_its_annotation() {
    let text = "@a\n) ]\nversion: u32 = 3\n";
    let ast = Cst::parse(text).build_ast(text);
    assert_eq!(ast.annotations.len(), 1);
    assert_eq!(names(&ast, text, &ast.roots.all[0]), [] as [&str; 0]);
}

#[test]
fn raw_annotation_shape() {
    let text = "@m(depth = 2, strict, s = \"x\")\nversion: u32 = 3\n";
    let ast = Cst::parse(text).build_ast(text);
    let a = ast.annotations.iter().next().unwrap().annotation();
    assert_eq!(&text[a.span()], "@m(depth = 2, strict, s = \"x\")");
    assert_eq!(&text[a.name()], "m");
    let args = a.arguments();
    assert_eq!(args.len(), 3);
    assert_eq!(&text[args[0].name()], "depth");
    assert_eq!(&text[args[0].span()], "depth = 2");
    assert!(matches!(args[0].value(), Some(Value::U64(v)) if v.value == 2));
    assert!(matches!(args[1], Argument::Flag(_)));
    assert_eq!(&text[args[1].span()], "strict");
    assert!(args[1].value().is_none());
    assert!(matches!(args[2].value(), Some(Value::String(s)) if s.value == "x"));
}

fn one(text: &str) -> (Ast, Span) {
    let ast = Cst::parse(text).build_ast(text);
    assert!(
        ast.diagnostics
            .iter()
            .all(|d| matches!(d.diagnostic, Diagnostic::MissingRootEntry { .. })),
        "{:#?}",
        ast.diagnostics
    );
    let target = ast.roots.all[0].target();
    (ast, target)
}

#[test]
fn argument_readers() {
    let text = "@m(u = 2, i = -2, f = 1.5, b = true, s = \"x\", h = 0xDEADBEEF, w = 0xDEADBEEFCAFE, n = null, flag)\nversion: u32 = 3\n";
    let (ast, target) = one(text);
    let table = ast.annotations.with_source(text);
    let a = table.on(&target).next().unwrap();

    assert_eq!(a.name(), "m");
    assert_eq!(a.arg::<u64>("u").unwrap(), Some(2));
    assert_eq!(a.arg::<u8>("u").unwrap(), Some(2));
    assert_eq!(a.arg::<i64>("u").unwrap(), Some(2));
    assert_eq!(a.arg::<i8>("i").unwrap(), Some(-2));
    assert_eq!(a.arg::<f32>("f").unwrap(), Some(1.5));
    assert_eq!(a.arg::<f32>("u").unwrap(), Some(2.0));
    assert_eq!(a.arg::<bool>("b").unwrap(), Some(true));
    assert_eq!(a.arg::<&str>("s").unwrap(), Some("x"));
    assert_eq!(a.arg::<BinHash>("h").unwrap(), Some(BinHash(0xDEADBEEF)));
    assert_eq!(a.arg::<WadHash>("h").unwrap(), Some(WadHash(0xDEADBEEF)));
    assert_eq!(
        a.arg::<WadHash>("w").unwrap(),
        Some(WadHash(0xDEADBEEFCAFE))
    );
    assert_eq!(a.arg::<BinHash>("s").unwrap(), Some(BinHash::hash_str("x")));
    assert_eq!(a.arg::<WadHash>("s").unwrap(), Some(WadHash::hash_str("x")));

    assert_eq!(a.arg::<u64>("missing").unwrap(), None);
    assert_eq!(a.arg::<u64>("n").unwrap(), None);
    assert_eq!(a.require::<u64>("u").unwrap(), 2);

    assert!(a.flag("flag").unwrap());
    assert!(a.flag("b").unwrap());
    assert!(!a.flag("missing").unwrap());
    assert!(!a.flag("n").unwrap());

    assert!(a
        .only(&["u", "i", "f", "b", "s", "h", "w", "n", "flag"])
        .is_ok());
}

#[test]
fn argument_errors_carry_spans() {
    let text = "@m(u = -1, big = 300, s = \"x\", flag, h = 0xDEADBEEFCAFE)\nversion: u32 = 3\n";
    let (ast, target) = one(text);
    let table = ast.annotations.with_source(text);
    let a = table.on(&target).next().unwrap();
    let span_of = |s: &str| {
        let start = text.find(s).unwrap() as u32;
        Span::new(start, start + s.len() as u32)
    };

    let e = a.arg::<u64>("u").unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::OutOfRange { ty: "u64" });
    assert_eq!(e.span, span_of("-1"));

    let e = a.arg::<u8>("big").unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::OutOfRange { ty: "u8" });

    let e = a.arg::<u64>("s").unwrap_err();
    assert_eq!(
        e.kind,
        ArgErrorKind::WrongType {
            expected: "integer"
        }
    );
    assert_eq!(e.span, span_of("\"x\""));

    let e = a.arg::<u64>("flag").unwrap_err();
    assert!(matches!(e.kind, ArgErrorKind::WrongType { .. }));
    assert_eq!(e.span, span_of("flag"));

    let e = a.arg::<BinHash>("h").unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::WrongType { expected: "hash" });

    let e = a.flag("s").unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::WrongType { expected: "bool" });

    let e = a.require::<u64>("missing").unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::Missing);
    assert_eq!(e.span, a.annotation().span());

    let e = a.only(&["u", "big"]).unwrap_err();
    assert_eq!(e.kind, ArgErrorKind::Unexpected);
    assert_eq!(e.span, span_of("s = \"x\""));
    assert_eq!(a.arg_name(&a.annotation().arguments()[2]), "s");
}

#[derive(Debug, Clone, PartialEq)]
enum Directive {
    Replace,
    Merge { depth: u64, strict: bool },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
enum DirectiveError {
    #[error(transparent)]
    Arg(#[from] ltk_ritobin::ast::annotation::ArgError),
    #[error("depth must be positive")]
    ZeroDepth,
}

impl FromAnnotation for Directive {
    type Error = DirectiveError;

    fn from_annotation(a: AnnotationRef<'_>) -> Result<Option<Self>, Spanned<Self::Error>> {
        match a.name() {
            "replace" => Ok(Some(Self::Replace)),
            "merge" => {
                a.only(&["depth", "strict"])?;
                let depth = a.require::<u64>("depth")?;
                if depth == 0 {
                    return Err(a.error(DirectiveError::ZeroDepth));
                }
                Ok(Some(Self::Merge {
                    depth,
                    strict: a.flag("strict")?,
                }))
            }
            _ => Ok(None),
        }
    }
}

const DIRECTIVES: &str = r#"#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {
    "a" = Foo {
        @replace
        x: u32 = 1
        @merge(depth = 2, strict)
        y: u32 = 2
        @other_tool(red)
        z: u32 = 3
        @merge(depth = 0)
        w: u32 = 4
        @merge(dpeth = 1)
        v: u32 = 5
    }
}
"#;

#[test]
fn resolve_splits_claimed_unclaimed_and_failed() {
    let ast = parse(DIRECTIVES);
    let resolved: Resolved<Directive, DirectiveError> = ast.resolve_annotations(DIRECTIVES);

    let typed = resolved.annotations();
    assert_eq!(typed.len(), 2);
    assert_eq!(
        typed.on(property(&ast, "x")).collect::<Vec<_>>(),
        [&Directive::Replace]
    );
    assert_eq!(
        typed.on(property(&ast, "y")).collect::<Vec<_>>(),
        [&Directive::Merge {
            depth: 2,
            strict: true
        }]
    );
    assert_eq!(typed.on(property(&ast, "z")).count(), 0);
    assert_eq!(typed.on(property(&ast, "w")).count(), 0);

    let unclaimed: Vec<&str> = resolved
        .unclaimed()
        .map(|a| &DIRECTIVES[a.annotation().name()])
        .collect();
    assert_eq!(unclaimed, ["other_tool"]);

    let errors = resolved.errors();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].value, DirectiveError::ZeroDepth);
    assert_eq!(&DIRECTIVES[errors[0].span], "@merge(depth = 0)");
    assert_eq!(
        errors[1].value,
        DirectiveError::Arg(ltk_ritobin::ast::annotation::ArgError {
            span: errors[1].span,
            kind: ArgErrorKind::Unexpected,
        })
    );
    assert_eq!(&DIRECTIVES[errors[1].span], "dpeth = 1");

    // the raw table is untouched
    assert_eq!(ast.annotations.len(), 5);
}

#[test]
fn resolve_with_takes_a_stateful_closure() {
    let ast = parse(DIRECTIVES);
    let mut seen = 0;
    let resolved = ast.annotations.resolve_with(DIRECTIVES, |a| {
        seen += 1;
        Ok::<_, Spanned<DirectiveError>>((a.name() == "replace").then_some(()))
    });
    assert_eq!(seen, 5);
    assert_eq!(resolved.annotations().len(), 1);
    assert_eq!(resolved.unclaimed().count(), 4);
    assert!(resolved.errors().is_empty());
}

#[test]
fn malformed_annotations_are_build_diagnostics() {
    let cases = [
        ("@m(a = 1, a = 2)\nversion: u32 = 3\n", "a = 2"),
        ("@m(a =)\nversion: u32 = 3\n", "a ="),
    ];
    for (text, at) in cases {
        let ast = Cst::parse(text).build_ast(text);
        let found = ast
            .diagnostics
            .iter()
            .find(|d| matches!(d.diagnostic, Diagnostic::AnnotationResolve(_)))
            .unwrap_or_else(|| panic!("{text:?}: {:#?}", ast.diagnostics));
        assert_eq!(&text[found.span], at, "{text:?}");
        assert!(ast.annotations.is_empty(), "{text:?}");
    }
}

#[test]
fn printing_round_trips_annotations() {
    let cst = Cst::parse(FILE);
    let mut printed = String::new();
    CstPrinter::new(FILE, &mut printed, Default::default())
        .print(&cst)
        .unwrap();
    let again = Cst::parse(&printed);
    assert!(again.errors.is_empty(), "{printed}");

    let before = parse(FILE);
    let after = again.build_ast(&printed);
    let names = |ast: &Ast, text: &str| -> Vec<String> {
        ast.annotations
            .iter()
            .map(|a| text[a.annotation().span()].to_string())
            .collect()
    };
    assert_eq!(names(&before, FILE), names(&after, &printed));
}

#[test]
fn an_annotation_inside_a_list_like_is_an_error() {
    let text = "#PROP_text\ntype: string = \"PROP\"\nversion: u32 = 3\nlinked: list[string] = {}\nentries: map[hash,embed] = {\n    \"a\" = Foo {\n        pos: vec3 = {\n            @a\n            1 2 3\n        }\n    }\n}\n";
    let ast = Cst::parse(text).build_ast(text);
    let unexpected: Vec<Span> = ast
        .diagnostics
        .iter()
        .filter_map(|d| match d.diagnostic {
            Diagnostic::UnexpectedItem { span, .. } => Some(span),
            _ => None,
        })
        .collect();
    let at = text.find("@a").unwrap() as u32;
    assert_eq!(unexpected, [Span::new(at, at + 2)]);
    assert_eq!(ast.diagnostics.len(), 1, "{:#?}", ast.diagnostics);
    // the value itself is intact
    assert!(matches!(
        property(&ast, "pos").value,
        Some(Value::Vector3(_))
    ));
}

#[test]
fn a_map_entry_annotation_is_on_the_entry_not_its_key_or_value() {
    let ast = parse(FILE);
    let text = FILE;
    let Some(Value::Map(lookup)) = property(&ast, "lookup").value.as_ref() else {
        panic!("lookup is a map");
    };
    let entry = &lookup.entries[0];
    let value = entry.value.as_ref().unwrap();

    assert_eq!(names(&ast, text, entry), ["on_map_entry"]);
    assert_eq!(names(&ast, text, &entry.key), [] as [&str; 0]);
    assert_eq!(names(&ast, text, value), [] as [&str; 0]);

    let inherited = |node: &dyn Annotatable| -> Vec<&str> {
        ast.annotations
            .with_source(text)
            .applying_to(&node.target())
            .map(|a| a.annotation().name())
            .collect()
    };
    assert_eq!(
        inherited(&entry.key),
        ["on_entries", "on_root_entry", "on_map", "on_map_entry"]
    );
    assert_eq!(inherited(value), inherited(&entry.key));
}

#[test]
fn an_entry_without_a_value_is_still_a_target() {
    let text = "@a
foo: u32
";
    let ast = Cst::parse(text).build_ast(text);
    assert_eq!(names(&ast, text, &ast.roots.all[0]), ["a"]);
}
