mod block_value;
mod class;
mod entry;
mod listlikes;
pub mod literals;
mod type_expr;
mod value;

use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{self, *},
            RitoTypeOrVirtual,
        },
        Value,
    },
    parse::Span,
    Node, RitoType,
};
