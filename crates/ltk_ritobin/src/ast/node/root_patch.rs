use ltk_hash::BinHash;
use ltk_meta::path::PropertyPath;

use crate::{
    ast::{hash::HashedLiteral, Value},
    parse::Span,
    Spanned,
};

/// One well-formed record of the `patches` root: `object = patch { path: .., value: .. }`.
///
/// Records keep the order they are written in, and several records may name the same object
/// and the same path.
#[derive(Debug, Clone)]
pub struct RootPatch {
    /// The path hash of the object the record patches.
    pub object_hash: HashedLiteral<BinHash>,
    /// The property inside that object. The span covers the string literal, quotes included.
    pub path: Spanned<PropertyPath>,
    /// The value the record writes, of the kind its `value` field declares.
    pub value: Value,
    /// The whole `object = patch { .. }` pair.
    pub span: Span,
}
