//! Parser for ritobin text format with CST output for better error reporting.

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
    use crate::{
        cst::{Cst, FlatErrors},
        print::CstPrinter,
        typecheck::visitor::TypeChecker,
    };

    fn assert_success(text: &str) {
        let cst = Cst::parse(text);

        let mut buf = String::new();
        cst.print(&mut buf, 0, text);

        println!("{buf}");

        assert!(cst.errors.is_empty(), "Top-level errors: {:#?}", cst.errors);
        let errors = FlatErrors::walk(&cst);
        assert!(errors.is_empty(), "Error trees: {:#?}", errors);
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

    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
entries: map[hash, embed] = {
    "foo" = Foo {
        guy: u32 = "asdasd"
    }
}
"#;
        let cst = Cst::parse(text);
        let errors = cst::FlatErrors::walk(&cst);

        let mut str = String::new();
        cst.print(&mut str, 0, text);
        eprintln!("text len: {}", text.len());
        eprintln!("{str}\n====== errors: ======\n");
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.kind);
        }

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
