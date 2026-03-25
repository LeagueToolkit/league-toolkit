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

pub trait Print {
    /// Print as ritobin code to the given writer (using default config, which prints hashes as hex).
    fn print_to_writer<W: fmt::Write>(&self, writer: &mut W) -> Result<usize, PrintError> {
        Self::print_to_writer_with_config::<W, ()>(self, writer, Default::default())
    }
    /// Print as ritobin code to the given writer, using the given config.
    fn print_to_writer_with_config<W: fmt::Write, H: HashProvider + Clone>(
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
    fn print_with_config<H: HashProvider + Clone>(
        &self,
        config: PrintConfig<H>,
    ) -> Result<String, PrintError> {
        let mut str = String::new();
        Self::print_to_writer_with_config(self, &mut str, config)?;
        Ok(str)
    }
}

impl Print for Bin {
    fn print_to_writer_with_config<W: fmt::Write, H: HashProvider + Clone>(
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
        cst::Cst,
        print::{config::PrintConfig, CstPrinter, WrapConfig},
    };

    fn assert_pretty(input: &str, is: &str, config: PrintConfig<()>) {
        let cst = Cst::parse(input);
        let mut str = String::new();

        cst.print(&mut str, 0, input);
        eprintln!("#### CST:\n{str}");

        let mut str = String::new();
        CstPrinter::new(input, &mut str, config)
            .print(&cst)
            .unwrap();

        pretty_assertions::assert_eq!(str.trim(), is.trim());
    }

    fn assert_pretty_rt(input: &str, config: PrintConfig<()>) {
        assert_pretty(input, input, config);
    }

    #[test]
    fn simple_list() {
        assert_pretty(
            r#" b  :  list [ i8, ] = {  3, 6 1 }"#,
            r#"b: list[i8] = { 3, 6, 1 }"#,
            PrintConfig::default(),
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
            PrintConfig::default(),
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
            PrintConfig::default(),
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
            PrintConfig::default(),
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
            PrintConfig::default(),
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
            PrintConfig::default(),
        );
    }

    #[test]
    fn zaahen_01() {
        assert_pretty_rt(
            r#"bankUnits: list2[embed] = {
    BankUnit {
        events: list[string] = {
            "PPlay_sfx_Zaahen_Dance3D_buffactivatePlay_sfx_Zaahen_Dance3D_buffactivatelay_sfx_Zaahen_Dance3D_buffactivate"
        }
    }
    BankUnit { }
}"#,
            PrintConfig::default(),
        );
    }

    #[test]
    fn broken_type_arg() {
        assert_pretty(
            r#"thing4: list[u3 2] = { 0, 0, 0, 0, 0, 0, 0 }"#,
            r#"thing4: list[u32] = { 0, 0, 0, 0, 0, 0, 0 }"#,
            PrintConfig::default(),
        );
    }

    #[test]
    fn inline_single_field_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage { image: string = "val" }"#,
            PrintConfig::default().wrap(WrapConfig::default().inline_structs(true)),
        );
    }
    #[test]
    fn inline_nested_single_field_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage {
    image: embed = Image { src: string = "val" }
}"#,
            PrintConfig::default().wrap(WrapConfig::default().inline_structs(true)),
        );
    }

    #[test]
    fn dont_inline_nested_single_field_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage {
    image: embed = Image {
        src: string = "val"
    }
}"#,
            PrintConfig::default().wrap(WrapConfig::default().inline_structs(false)),
        );
    }

    #[test]
    fn dont_inline_simple_list_or_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage {
    b: list[i8] = {
        3
        6
        1
    }
    image: embed = Image {
        src: string = "val"
    }
}"#,
            PrintConfig::default().wrap(
                WrapConfig::default()
                    .inline_lists(false)
                    .inline_structs(false),
            ),
        );
    }
    #[test]
    fn dont_inline_simple_list_and_inline_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage {
    b: list[i8] = {
        3
        6
        1
    }
    image: embed = Image { src: string = "val" }
}"#,
            PrintConfig::default().wrap(
                WrapConfig::default()
                    .inline_lists(false)
                    .inline_structs(true),
            ),
        );
    }
    #[test]
    fn inline_simple_list_and_dont_inline_struct() {
        assert_pretty_rt(
            r#"loadscreen: embed = CensoredImage {
    b: list[i8] = { 3, 6, 1 }
    image: embed = Image {
        src: string = "val"
    }
}"#,
            PrintConfig::default().wrap(
                WrapConfig::default()
                    .inline_lists(true)
                    .inline_structs(false),
            ),
        );
    }

    #[test]
    fn unterminated_string() {
        assert_pretty_rt(
            r#"ConformToPathRigPoseModifierData {
    mStartingJointName: hash = "L_Clavicle
    mEndingJointName: hash = "l_hand"
    mDefaultMaskName: hash = 0x7136e1bc
    mMaxBoneAngle: f32 = 115
    mDampingValue: f32 = 8
    mVelMultiplier: f32 = 0
    mFrequency: f32 = 20
}"#,
            PrintConfig::default(),
        )
    }
}
