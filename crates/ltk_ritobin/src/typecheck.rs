//! CST visitor that validates and resolves types into Bin's
//!
//! You should only use types from here directly if you know what you are doing - see
//! [`crate::Cst::build_bin`]
//!
//! TODO: better explanation of the type checking impl

pub mod visitor;

#[cfg(test)]
mod test {
    use glam::{Vec3, Vec4};
    use ltk_meta::{
        property::{values, NoMeta},
        Bin, BinObject, ObjectBuilder, PropertyKind,
    };

    use crate::Cst;

    fn assert<F: Fn(ObjectBuilder) -> ObjectBuilder>(input: &str, is: F) {
        let input = format!(
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
        );

        let cst = Cst::parse(&input);
        let mut str = String::new();

        cst.print(&mut str, &input);
        eprintln!("#### CST:\n{str}");

        let (bin, errs) = cst.build_bin(&input);

        assert!(errs.is_empty(), "Typecheck errors: {:#?}", errs);

        let obj = (is)(BinObject::<NoMeta>::builder(0xDEADBEEF, 0x1234123)).build();
        pretty_assertions::assert_eq!(bin, Bin::builder().object(obj).build());
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
}
