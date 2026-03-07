//! Parser for ritobin text format with CST output for better error reporting.

pub mod cst;

pub mod error;
pub use error::*;

pub mod parser;
pub mod real;
pub mod tokenizer;
pub use tokenizer::{Token, TokenKind};

pub mod impls;

mod span;
pub use span::Span;

pub fn parse(text: &str) -> cst::Cst {
    let tokens = tokenizer::lex(text);
    let mut p = parser::Parser::new(text, tokens);
    impls::file(&mut p);
    p.build_tree()
}

#[cfg(test)]
mod test {
    use crate::{print::Printer, typecheck::visitor::TypeChecker};

    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
entries: map[hash,embed] = {
    "myPath" = VfxEmitter {
        a: string = "hello"
        b: list[i8] = {3 6 1}
    }
}
"#;
        let cst = parse(text);
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
        let cst = parse(text);

        let mut str = String::new();
        cst.print(&mut str, 0, text);

        println!("============= CST ===========");
        println!("{str}");

        let mut str = String::new();
        let size = 100;
        Printer::new(text, &mut str, size).print(&cst).unwrap();

        println!("{}", "*".repeat(size));
        println!("========\n{str}");
    }
}
