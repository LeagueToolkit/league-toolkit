//! CST visitor that validates and resolves types into Bin's
//!
//! You should only use types from here directly if you know what you are doing - see
//! [`crate::Cst::build_bin`]
//!
//! TODO: better explanation of the type checking impl

pub mod diagnostics;
pub mod ir;
pub mod state;

mod collect;
mod resolve;
mod trace;
mod vecmath;
mod walk;

pub use state::TypeChecker;

#[cfg(test)]
mod test {
    use glam::{Vec3, Vec4};
    use ltk_meta::{
        property::{values, NoMeta},
        Bin, BinObject, ObjectBuilder, PropertyKind,
    };

    use crate::{
        typecheck::diagnostics::{Diagnostic, DiagnosticWithSpan, RootKind},
        Cst,
    };

    fn wrap(input: &str) -> String {
        format!(
            r#"
#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {{}}
entries: map[hash,embed] = {{
    0xDEADBEEF = 0x1234123 {{
        {input}
    }}
}}"#
        )
    }

    fn assert<F: Fn(ObjectBuilder) -> ObjectBuilder>(input: &str, is: F) {
        let input = wrap(input);

        let cst = Cst::parse(&input);
        let mut str = String::new();

        cst.print(&mut str, &input);
        eprintln!("#### CST:\n{str}");

        let (bin, errs) = cst.build_bin(&input);

        assert!(errs.is_empty(), "Typecheck errors: {:#?}", errs);

        let obj = (is)(BinObject::<NoMeta>::builder(0xDEADBEEF, 0x1234123)).build();
        pretty_assertions::assert_eq!(bin, Bin::builder().object(obj).build());
    }

    /// Builds a full object body (see [`wrap`]) from `input` and returns the
    /// typecheck diagnostics without asserting they're empty - for exercising
    /// error paths.
    fn build_errs(input: &str) -> Vec<DiagnosticWithSpan> {
        let input = wrap(input);
        let cst = Cst::parse(&input);
        let (_, errs) = cst.build_bin(&input);
        errs
    }

    #[test]
    fn option_coerce() {
        assert(r#"0x1: option[vec3] = { 0.5, 5.3, -0.20 }"#, |obj| {
            obj.property(
                0x1,
                values::Optional::from(values::Vector3::from(Vec3::new(0.5, 5.3, -0.2))),
            )
        });
    }

    #[test]
    fn list() {
        assert(
            r#"
        values: list[vec4] = {
            { 1, 1, 1, 1 }
            { 1, 1, 1, 1 }
            { 1, 1, 1, 0 }
        }
        "#,
            |obj| {
                obj.property(
                    0x34474c3b,
                    values::Container::from_iter([
                        values::Vector4::from(Vec4::new(1., 1., 1., 1.)),
                        values::Vector4::from(Vec4::new(1., 1., 1., 1.)),
                        values::Vector4::from(Vec4::new(1., 1., 1., 0.)),
                    ]),
                )
            },
        );
    }

    #[test]
    fn u8_map() {
        assert(
            r#"
        0xe6d60f41: map[u8,string] = {
            1 = "hello"
        }
        "#,
            |obj| {
                obj.property(
                    0xe6d60f41,
                    values::Map::new(
                        PropertyKind::U8,
                        PropertyKind::String,
                        vec![(
                            values::U8::from(1).into(),
                            values::String::from("hello").into(),
                        )],
                    )
                    .unwrap(),
                )
            },
        );
    }

    #[test]
    fn matrix() {
        assert(
            r#"
        0x1: mtx44 = {
            0.1, 0.2, 0.3, 0.4,
            1.1, 1.2, 1.3, 1.4,
            2.1, 2.2, 2.3, 2.4,
            3.1, 3.2, 3.3, 3.4
        }
        "#,
            |obj| {
                obj.property(
                    0x1,
                    values::Matrix44::from(glam::Mat4::from_cols_array_2d(&[
                        [0.1, 1.1, 2.1, 3.1],
                        [0.2, 1.2, 2.2, 3.2],
                        [0.3, 1.3, 2.3, 3.3],
                        [0.4, 1.4, 2.4, 3.4],
                    ])),
                )
            },
        );
    }

    #[test]
    fn numeric_parse_error() {
        let errs = build_errs("0x1: u8 = 999999");
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(
            matches!(
                errs[0].diagnostic,
                Diagnostic::ParseNumericError {
                    expected: PropertyKind::U8,
                    ..
                }
            ),
            "{:#?}",
            errs[0]
        );
    }

    #[test]
    fn subtype_count_mismatch_too_many() {
        // Container/list takes exactly 1 subtype
        let errs = build_errs("0x1: list[u8,u8] = {}");
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(
            matches!(
                errs[0].diagnostic,
                Diagnostic::SubtypeCountMismatch {
                    expected: 1,
                    got: 2,
                    ..
                }
            ),
            "{:#?}",
            errs[0]
        );
    }

    #[test]
    fn subtype_count_mismatch_too_few() {
        // Map takes exactly 2 subtypes
        let errs = build_errs("0x1: map[u8] = {}");
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(
            matches!(
                errs[0].diagnostic,
                Diagnostic::SubtypeCountMismatch {
                    expected: 2,
                    got: 1,
                    ..
                }
            ),
            "{:#?}",
            errs[0]
        );
    }

    #[test]
    fn missing_linked_root_entry_reports_diagnostic_without_panicking() {
        let input = r#"
type: string = "PROP"
version: u32 = 3
entries: map[hash,embed] = {}
"#;
        let cst = Cst::parse(input);
        let (_, errs) = cst.build_bin(input);
        assert!(
            errs.iter().any(|e| matches!(
                e.diagnostic,
                Diagnostic::MissingRootEntry {
                    root_kind: RootKind::Linked
                }
            )),
            "{errs:#?}"
        );
    }

    #[test]
    fn missing_entries_root_entry_reports_diagnostic_without_panicking() {
        let input = r#"
type: string = "PROP"
version: u32 = 3
linked: list[string] = {}
"#;
        let cst = Cst::parse(input);
        let (_, errs) = cst.build_bin(input);
        assert!(
            errs.iter().any(|e| matches!(
                e.diagnostic,
                Diagnostic::MissingRootEntry {
                    root_kind: RootKind::Entries
                }
            )),
            "{errs:#?}"
        );
    }

    #[test]
    fn missing_type_root_entry_reports_type_not_version() {
        let input = r#"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
"#;
        let cst = Cst::parse(input);
        let (_, errs) = cst.build_bin(input);
        assert!(
            errs.iter().any(|e| matches!(
                e.diagnostic,
                Diagnostic::MissingRootEntry {
                    root_kind: RootKind::Type
                }
            )),
            "{errs:#?}"
        );
    }

    #[test]
    fn invalid_type_root_entry_reports_type_not_version() {
        let input = r#"
type: u32 = 3
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
"#;
        let cst = Cst::parse(input);
        let (_, errs) = cst.build_bin(input);
        assert!(
            errs.iter().any(|e| matches!(
                e.diagnostic,
                Diagnostic::InvalidRootEntryType {
                    root_kind: RootKind::Type,
                    ..
                }
            )),
            "{errs:#?}"
        );
    }
}
