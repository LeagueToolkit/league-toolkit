//! Ritobin parser, tokenizer & other relevant types.
//!
//! To actually parse text, see [`crate::Cst::parse`].
//!
//! Parsing is purely syntactic: it builds the CST without performing any semantic
//! analysis such as type resolution or validation beyond what is needed to form a valid CST.
//!
//! See [`crate::Cst::build_bin`] & [`crate::typecheck`] for further typechecking/bin construction.

mod error;
pub use error::*;

mod parser;
pub use parser::*;

pub mod tokenizer;
pub use tokenizer::{Token, TokenKind};

pub mod impls;

mod span;
pub use span::Span;

use crate::cst;

#[cfg(test)]
mod test {
    use crate::{cst::Cst, print::CstPrinter, typecheck::visitor::TypeChecker};

    fn assert_success(text: &str) -> Cst {
        let cst = Cst::parse(text);

        let mut buf = String::new();
        cst.print(&mut buf, 0, text);

        println!("{buf}");

        assert!(cst.errors.is_empty(), "Parse errors: {:#?}", cst.errors);
        cst
    }

    fn assert_fail(text: &str) -> Cst {
        let cst = Cst::parse(text);

        let mut buf = String::new();
        cst.print(&mut buf, 0, text);

        println!("{buf}");
        assert!(!cst.errors.is_empty(), "Parsed successfully",);
        cst
    }

    #[test]
    fn comments() {
        assert_success(
            r#"
#PROP_text
type: string = "my_str"
# version: u32 = 3
linked: list[string] = { }
entries: map[hash, embed] = {
    "foo" = Bar {
        # asdasd
    }
}
        "#,
        );
    }

    #[ignore = "Nice to have"]
    #[test]
    fn naked_class() {
        let text = r#"
        entries: map[hash, embed] = {
            "thing" = ClassThing { ooo }
        }
        "#;
        let cst = assert_success(text);

        let (_bin, errors) = cst.build_bin(text);
        assert!(
            !errors.is_empty(),
            "There should be an error for the naked 'ooo' in the class block"
        );
    }

    #[test]
    fn smoke_test() {
        let text = r#"
#PROP_text
entries: map[hash, embed] = {
    "foo" = Foo {
        guy: u32 = "asdasd"
    }
}

"#;
        let cst = Cst::parse(text);

        let mut str = String::new();
        cst.print(&mut str, 0, text);
        eprintln!("text len: {}", text.len());
        eprintln!("{str}\n====== errors: ======\n");

        let errors = &cst.errors;
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.kind);
        }

        assert!(errors.is_empty());

        let mut checker = TypeChecker::new(text);
        cst.walk(&mut checker);

        // let (mut roots, errors) = checker.into_parts();
        let (tree, errors) = checker.collect_to_bin();

        eprintln!("{str}\n====== type errors: ======\n");
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.diagnostic);
        }

        eprintln!("==== FINAL TREE =====\n{tree:#?}");
    }

    #[test]
    fn writer_test() {
        let text = r#"
entries: map[hash,embed] = {
    "myPath" = VfxEmitter {
        a: string = "hello"
        b: list[i8] = {3 6 1}
    }
    "cock" = VfxEmitterDefinitionData {
                rate: embed = ValueFloat {
                    constantValue: f32 = 1
                }
                particleLifetime: embed = ValueFloat {
                    constantValue: f32 = 1
                }
                particleLinger: option[f32] = {
                    2
                }
                lifetime: option[f32] = {
                    1
                }
                emitterName: string = "JudgementCut"
                bindWeight: embed = ValueFloat {
                    constantValue: f32 = 1
                }
                primitive: pointer = VfxPrimitiveMesh {
                    mMesh: embed = VfxMeshDefinitionData {
                        mMeshName: string = "ASSETS/Characters/viego/Skins/base/judgementcut.skn"
                        mMeshSkeletonName: string = "ASSETS/Characters/viego/Skins/base/judgementcut.skl"
                        mAnimationName: string = "ASSETS/Characters/viego/Skins/base/judgementcut.anm"
                    }
                }
                birthScale0: embed = ValueVector3 {
                    constantValue: vec3 = { 15, 15, 15 }
                }
                blendMode: u8 = 1
                disableBackfaceCull: bool = true
                miscRenderFlags: u8 = 1
                texture: string = "ASSETS/Characters/viego/Skins/base/slashes.dds"
                particleUVScrollRate: embed = IntegratedValueVector2 {
                    constantValue: vec2 = { 1, 0 }
                    dynamics: pointer = VfxAnimatedVector2fVariableData {
                        times: list[f32] = {
                            0
                        }
                        values: list[vec2] = {
                            { 1, 0 }
                        }
                    }
                }
            }
}
"#;
        let cst = Cst::parse(text);

        let mut str = String::new();
        cst.print(&mut str, 0, text);

        println!("============= CST ===========");
        println!("{str}");

        let mut str = String::new();
        CstPrinter::new(text, &mut str, Default::default())
            .print(&cst)
            .unwrap();

        println!("{}", "*".repeat(80));
        println!("========\n{str}");
    }
}
