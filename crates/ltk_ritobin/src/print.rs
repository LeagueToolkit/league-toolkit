#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error(transparent)]
    FmtError(#[from] fmt::Error),
}

use std::fmt::{self};

use ltk_meta::Bin;

use crate::hashes::HashProvider;

pub mod command;
pub mod visitor;

mod config;
pub use config::*;

mod printers;
pub use printers::*;

// pub fn print_bin(bin: &Bin, width: usize) -> Result<String, PrintError> {
//     let mut str = String::new();
//
//     let (buf, cst) = bin_to_cst(bin);
//
//     let mut tmp = String::new();
//     cst.print(&mut tmp, 0, &buf);
//     println!("[print] cst:\n{tmp}");
//
//     Printer::new(&buf, &mut str, width).print(&cst)?;
//
//     Ok(str)
// }

pub trait Print {
    /// Print as ritobin code to the given writer (using default config, which prints hashes as hex).
    fn print_to_writer<W: fmt::Write>(&self, writer: &mut W) -> Result<usize, PrintError> {
        Self::print_to_writer_with_config::<W, ()>(self, writer, Default::default())
    }
    /// Print as ritobin code to the given writer, using the given config.
    fn print_to_writer_with_config<W: fmt::Write, H: HashProvider>(
        &self,
        writer: &mut W,
        config: PrintConfig<H>,
    ) -> Result<usize, PrintError>;

    /// Print as ritobin code to a string (using default config, which prints hashes as hex).
    fn print(&self) -> Result<String, PrintError> {
        let mut str = String::new();
        Self::print_to_writer(self, &mut str)?;
        Ok(str)
    }
    /// Print as ritobin code to a string, using the given config.
    fn print_with_config<H: HashProvider>(
        &self,
        config: PrintConfig<H>,
    ) -> Result<String, PrintError> {
        let mut str = String::new();
        Self::print_to_writer_with_config(self, &mut str, config)?;
        Ok(str)
    }
}

impl Print for Bin {
    fn print_to_writer_with_config<W: fmt::Write, H: HashProvider>(
        &self,
        writer: &mut W,
        config: PrintConfig<H>,
    ) -> Result<usize, PrintError> {
        BinPrinter::new().with_config(config).print(self, writer)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        parse::parse,
        print::{config::PrintConfig, CstPrinter},
    };

    fn assert_pretty(input: &str, is: &str, size: usize) {
        let cst = parse(input);
        let mut str = String::new();

        cst.print(&mut str, 0, input);
        eprintln!("#### CST:\n{str}");

        let mut str = String::new();
        CstPrinter::new(
            input,
            &mut str,
            PrintConfig {
                indent_size: 4,
                line_width: size,
                hashes: (),
            },
        )
        .print(&cst)
        .unwrap();

        pretty_assertions::assert_eq!(str.trim(), is.trim());
    }

    #[test]
    fn simple_list() {
        assert_pretty(
            r#" b  :  list [ i8, ] = {  3, 6 1 }"#,
            r#"b: list[i8] = { 3, 6, 1 }"#,
            80,
        );
    }

    #[test]
    fn vec2_list() {
        assert_pretty(
            r#" vec2List  :  list [ vec2, ] = {  {3, 6} {1 10000} }"#,
            r#"vec2List: list[vec2] = {
    { 3, 6 } 
    { 1, 10000 } 
}"#,
            80,
        );
    }

    #[test]
    fn class_list() {
        assert_pretty(
            r#" classList  :  list2[ embed] = {  MyClass {a: string = "hello"}
            FooClass {b: string = "foo"}}"#,
            r#"classList: list2[embed] = {
    MyClass {
        a: string = "hello"
    }
    FooClass {
        b: string = "foo"
    }
}"#,
            80,
        );
    }

    #[test]
    fn simple_class_embed() {
        assert_pretty(
            r#"skinUpgradeData: embed = skinUpgradeData { 
            mGearSkinUpgrades: list[link] = { 0x3b9c7079, 0x17566805 }
        }"#,
            r#"skinUpgradeData: embed = skinUpgradeData {
    mGearSkinUpgrades: list[link] = { 0x3b9c7079, 0x17566805 }
}"#,
            80,
        );
    }

    #[test]
    fn long_string_list() {
        assert_pretty(
            r#"
linked: list[string] = { "DATA/Characters/Viego/Viego.bin"
    "DATA/Viego_Skins_Skin0_Skins_Skin1_Skins_Skin10_Skins_Skin11_Skins_Skin12_Skins_Skin13_Skins_Skin14_Skins_Skin15_Skins_Skin16_Skins_Skin17_Skins_Skin18_Skins_Skin2_Skins_Skin3_Skins_Skin4_Skins_Skin43_Skins_Skin5_Skins_Skin6_Skins_Skin7_Skins_Skin8.bin"
}
"#,
            r#"linked: list[string] = {
    "DATA/Characters/Viego/Viego.bin"
    "DATA/Viego_Skins_Skin0_Skins_Skin1_Skins_Skin10_Skins_Skin11_Skins_Skin12_Skins_Skin13_Skins_Skin14_Skins_Skin15_Skins_Skin16_Skins_Skin17_Skins_Skin18_Skins_Skin2_Skins_Skin3_Skins_Skin4_Skins_Skin43_Skins_Skin5_Skins_Skin6_Skins_Skin7_Skins_Skin8.bin"
}"#,
            80,
        );
    }

    #[test]
    fn list_of_list_of_link() {
        assert_pretty(
            r#"BorderAugments: list2[embed] = {
    0x4a70b12c {
        AugmentGroup: list2[link] = { 0x383e4602 }
    }
}"#,
            r#"BorderAugments: list2[embed] = {
    0x4a70b12c {
        AugmentGroup: list2[link] = { 0x383e4602 }
    }
}"#,
            80,
        );
    }
}
