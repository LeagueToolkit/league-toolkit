//! A second, independent typechecker producing a persisted, queryable tree.
//!
//! `Cst::build_bin` (driven by [`crate::typecheck`]) stays the untouched, zero-ceremony path for
//! anyone who just wants a [`ltk_meta::Bin`] - fast, no intermediate tree, exactly what it's
//! always been. This module is for programmatic consumers instead: lints, an LSP, a refactoring
//! tool - anything that wants a persisted, typed, queryable structure it can walk itself. It's a
//! second implementation of the same coercion/diagnostic rules, not a shared pipeline with
//! `typecheck` - see the crate's design notes for why, and `tests/differential.rs` for how the
//! two are kept from silently drifting apart.
//!
//! [`build::build`] walks the CST once and produces the whole tree already resolved - there is
//! no separate "unresolved AST" stage, because nothing needs one: both a batch export (`to_bin`)
//! and an LSP's hover/diagnostics always want the fully resolved tree anyway.
//!
//! Node types ([`nodes::AstValue`], [`nodes::AstStruct`], [`nodes::AstProperty`]) deliberately
//! don't reuse `ltk_meta::PropertyValueEnum`/`values::Struct` for anything but true leaves:
//! `Struct<M>`'s `IndexMap<BinHash, _>` properties lose each property name's own span, which is
//! exactly the gap this module exists to close.

pub mod build;
mod listlikes;
pub mod nodes;
pub mod query;
mod to_bin;

pub use build::{build, Ast, AstObject};
pub use nodes::{AstProperty, AstStruct, AstValue, Spanned};
pub use query::Located;
