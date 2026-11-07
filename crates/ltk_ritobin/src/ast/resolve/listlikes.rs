use std::ops::ControlFlow;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{Diagnostic as D, ListLike, MaybeSpanDiag, TypeMismatch},
        Value,
    },
    cst::{Child, Cst, Kind, Node},
    ScalarType, Spanned,
};
use crate::{rito, span::Span};

use super::literals::ExpectedType;

/// Iterate a list-like (`vec*`,`rgba`,`mtx44`) block's children, emitting resolved [`Value`]s.
/// This does **NOT** handle actual lists (`list`/`list2`).
struct ListLikeIter<'a, 'b, 'c> {
    ctx: &'a mut Builder<'b>,
    cst: &'c Cst,
    span: Span,
    type_span: Option<Span>,
    count: u8,
    /// The list-like being read, for diagnosing items it cannot hold.
    kind: ScalarType,

    children: std::slice::Iter<'c, Child>,
    /// Iterator over the children of a `BlockLiteral` (for handling mtx44 rows).
    nested_block: Option<std::slice::Iter<'c, Child>>,
}

impl<'c> ListLikeIter<'_, '_, 'c> {
    fn resolve_numeric(
        &mut self,
        node: &Node,
        expected: ScalarType,
    ) -> Result<Value, MaybeSpanDiag> {
        self.count += 1;
        self.span = node.span;
        self.ctx
            .resolve_numeric(node, expected, self.type_span)
            .map_err(MaybeSpanDiag::from)
    }

    fn visit_node(
        &mut self,
        child: &Child,
        expected: ScalarType,
    ) -> ControlFlow<Result<Value, MaybeSpanDiag>> {
        let Some(node) = child.tree(self.cst) else {
            return ControlFlow::Continue(());
        };
        match node.kind {
            Kind::Comment => ControlFlow::Continue(()),
            // mtx44's can be represented as a block with 4 block children,
            // one for each row of the matrix
            Kind::BlockLiteral | Kind::Block => {
                if self.nested_block.is_some() {
                    self.ctx.push(
                        D::UnexpectedItem {
                            span: node.span,
                            parent: self.kind.into(),
                            expected: crate::ItemShape::Value,
                        }
                        .unwrap(),
                    );
                } else {
                    self.nested_block
                        .replace(node.children.get(self.cst).iter());
                }
                ControlFlow::Continue(())
            }
            Kind::Annotation => {
                self.ctx.push(
                    D::UnexpectedItem {
                        span: node.span,
                        parent: self.kind.into(),
                        expected: crate::ItemShape::Value,
                    }
                    .unwrap(),
                );
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Break(self.resolve_numeric(node, expected)),
        }
    }
    fn next_value(&mut self, expected: ScalarType) -> Option<Result<Value, MaybeSpanDiag>> {
        loop {
            // take from any nested blocks first
            let child = match self.nested_block.as_mut().and_then(|it| it.next()) {
                Some(child) => child,
                None => {
                    self.nested_block = None;
                    self.children.next()?
                }
            };
            match self.visit_node(child, expected) {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(v) => return Some(v),
            }
        }
    }

    fn expect_next(
        &mut self,
        expected_kind: ScalarType,
        expected: ListLike,
    ) -> Result<Value, MaybeSpanDiag> {
        match self.next_value(expected_kind) {
            Some(v) => v,
            None => Err(D::NotEnoughItems {
                span: self.span,
                got: self.count,
                expected,
            }
            .into()),
        }
    }

    fn read_floats<const N: usize>(
        &mut self,
        expected: ListLike,
    ) -> Result<[f32; N], MaybeSpanDiag> {
        let mut out = [0.0f32; N];
        for slot in &mut out {
            *slot = match self.expect_next(ScalarType::F32, expected)? {
                Value::F32(v) => v.value,
                other => {
                    return Err(D::from(TypeMismatch {
                        span: other.span(),
                        expected: rito!(F32).into(),
                        expected_span: self.type_span,
                        got: other.rito_type().into(),
                    })
                    .into())
                }
            };
        }
        Ok(out)
    }

    fn read_u8s<const N: usize>(&mut self, expected: ListLike) -> Result<[u8; N], MaybeSpanDiag> {
        let mut out = [0u8; N];
        for slot in &mut out {
            *slot = match self.expect_next(ScalarType::U8, expected)? {
                Value::U8(v) => v.value,
                other => {
                    return Err(D::from(TypeMismatch {
                        span: other.span(),
                        expected: rito!(U8).into(),
                        expected_span: self.type_span,
                        got: other.rito_type().into(),
                    })
                    .into())
                }
            };
        }
        Ok(out)
    }
}

impl<'a> Builder<'a> {
    /// `node` is the `ListItem` wrapping the literal.
    pub(crate) fn resolve_numeric(
        &mut self,
        node: &Node,
        expected: ScalarType,
        hint_span: Option<Span>,
    ) -> Result<Value, D> {
        self.resolve_value(node, Some(ExpectedType::from(expected)), hint_span)
    }

    pub(crate) fn resolve_listlike_fallable(
        &mut self,
        block: &Node,
        kind: ScalarType,
        type_span: Option<Span>,
    ) -> Result<Value, MaybeSpanDiag> {
        let cst = self.cst();
        let span = block.span;
        let mut items = ListLikeIter {
            ctx: self,
            children: block.children.get(cst).iter(),
            nested_block: None,
            cst,
            span,
            type_span,
            count: 0,
            kind,
        };

        let value = match kind {
            ScalarType::Vector2 => {
                let [x, y] = items.read_floats::<2>(ListLike::Vec2)?;
                Value::Vector2(Spanned::new(span, [x, y].into()))
            }
            ScalarType::Vector3 => {
                let [x, y, z] = items.read_floats::<3>(ListLike::Vec3)?;
                Value::Vector3(Spanned::new(span, [x, y, z].into()))
            }
            ScalarType::Vector4 => {
                let [x, y, z, w] = items.read_floats::<4>(ListLike::Vec4)?;
                Value::Vector4(Spanned::new(span, [x, y, z, w].into()))
            }
            ScalarType::Color => {
                let [r, g, b, a] = items.read_u8s::<4>(ListLike::Color)?;
                Value::Color(Spanned::new(span, ltk_primitives::Color { r, g, b, a }))
            }
            ScalarType::Matrix44 => {
                let x_axis = items.read_floats::<4>(ListLike::Mat44)?;
                let y_axis = items.read_floats::<4>(ListLike::Mat44)?;
                let z_axis = items.read_floats::<4>(ListLike::Mat44)?;
                let w_axis = items.read_floats::<4>(ListLike::Mat44)?;
                let mat = glam::Mat4::from_cols(
                    x_axis.into(),
                    y_axis.into(),
                    z_axis.into(),
                    w_axis.into(),
                )
                .transpose();
                Value::Matrix44(Spanned::new(span, mat))
            }
            _ => unreachable!("resolve_listlike called with a non-listlike kind"),
        };

        let expected = match kind {
            ScalarType::Vector2 => ListLike::Vec2,
            ScalarType::Vector3 => ListLike::Vec3,
            ScalarType::Vector4 => ListLike::Vec4,
            ScalarType::Color => ListLike::Color,
            ScalarType::Matrix44 => ListLike::Mat44,
            _ => unreachable!(),
        };
        if let Some(extra) = items.next_value(ScalarType::F32) {
            let extra = extra?;
            let count = 1 + items.children.count();
            return Err(D::TooManyItems {
                span: extra.span(),
                extra: count as _,
                expected,
            }
            .into());
        }
        Ok(value)
    }

    /// Resolves a `Block`/`ListItemBlock` node whose body is a flat list of bare numbers into
    /// one packed [`AstValue`] of `kind` (`Vector2`/`Vector3`/`Vector4`/`Color`/`Matrix44`).
    ///
    /// Any errors in resolution are automatically pushed, and a [`Value::Unresolved`] is returned.
    pub(super) fn resolve_listlike(
        &mut self,
        block: &Node,
        kind: ScalarType,
        type_span: Option<Span>,
    ) -> Value {
        match self.resolve_listlike_fallable(block, kind, type_span) {
            Ok(value) => value,

            Err(e) => {
                self.push(e.fallback(block.span));
                Value::Unresolved {
                    kind: kind.into(),
                    span: block.span,
                }
            }
        }
    }
}
