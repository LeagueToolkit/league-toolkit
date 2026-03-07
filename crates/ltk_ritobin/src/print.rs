use crate::parse::{
    cst::{visitor::Visit, Cst, Kind, Visitor},
    Span, TokenKind,
};

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error(transparent)]
    FmtError(#[from] fmt::Error),
}

use std::fmt::{self, Write};

mod visitor;

pub struct Printer<'a, W: Write> {
    visitor: visitor::Printer<'a, W>,
}

impl<'a, W: Write> Printer<'a, W> {
    pub fn new(src: &'a str, out: W, width: usize) -> Self {
        Self {
            visitor: visitor::Printer::new(src, out, width),
        }
    }

    pub fn print(mut self, cst: &Cst) -> Result<(), PrintError> {
        cst.walk(&mut self.visitor);
        self.visitor.flush()?;
        if let Some(e) = self.visitor.error {
            return Err(e);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::{parse::parse, print::Printer};

    fn assert_pretty(input: &str, is: &str, size: usize) {
        let cst = parse(input);
        let mut str = String::new();

        cst.print(&mut str, 0, input);
        eprintln!("#### CST:\n{str}");

        let mut str = String::new();
        Printer::new(input, &mut str, size).print(&cst).unwrap();

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
    fn nested_list() {
        assert_pretty(
            r#" nestedList  :  list [ vec2, ] = {  {3, 6} {1 10000} }"#,
            r#"nestedList: list[vec2] = {
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
}
