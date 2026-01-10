use indexmap::IndexMap;
use ltk_meta::{BinPropertyKind, PropertyValueEnum};

use crate::{
    parse::{
        cst::{self, visitor::Visit, Cst, Kind, Visitor},
        Span, TokenKind,
    },
    typecheck::RitobinName,
};

pub struct TypeChecker<'a> {
    text: &'a str,
    root: IndexMap<String, PropertyValueEnum>,
    current: Option<(String, PropertyValueEnum)>,
}

pub enum Diagnostic {
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),
    MissingToken(TokenKind),
    UnknownType(Span),
}
use Diagnostic::*;

pub struct RitoType {
    pub base: BinPropertyKind,
}

pub enum Statement {
    KeyValue {
        key: Span,
        value: Span,
        kind: Option<RitoType>,
    },
}

pub fn resolve_rito_type<'a>(text: &'a str, tree: &Cst) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.iter();

    let base = c
        .find_map(|c| c.token().filter(|t| t.kind == TokenKind::Name))
        .ok_or(MissingToken(TokenKind::Name))?;

    let base =
        BinPropertyKind::from_ritobin_name(&text[base.span]).ok_or(UnknownType(base.span))?;

    Ok(RitoType { base })
}

pub fn resolve_entry<'a>(text: &'a str, tree: &Cst) -> Result<(), Diagnostic> {
    let mut c = tree.children.iter();

    let key = c
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeExpr))
        .ok_or(MissingTree(Kind::EntryKey))?;

    let kind = c
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::EntryKey))
        .map(|t| resolve_rito_type(text, t))
        .transpose()?;

    let value = c
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::EntryValue))
        .ok_or(MissingTree(Kind::EntryValue))?;

    Ok(())
}

pub fn resolve_list(tree: &Cst) -> Result<(), Diagnostic> {
    Ok(())
}

impl Visitor for TypeChecker {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Skip;
        }

        match self.current.as_mut() {
            Some((name, value)) => {}
            None => {
                if tree.kind != cst::Kind::Entry {
                    return Visit::Skip;
                };

                let mut children = tree.children.iter();

                // let args = children.next().ok_or();
            }
        }

        Visit::Continue
    }

    fn exit_tree(&mut self, tree: &cst::Cst) -> Visit {
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        match self.current.take() {
            Some((name, value)) => {
                // TODO: warn when shadowed
                let _existing = self.root.insert(name, value);
            }
            None => {
                eprintln!("exit tree with no current?");
            }
        }

        Visit::Continue
    }
}
