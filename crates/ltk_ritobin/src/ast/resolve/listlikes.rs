use ltk_meta::{property::values, PropertyKind};

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{Diagnostic, ListLike, MaybeSpanDiag, RitoTypeOrVirtual},
        Value,
    },
    cst::{Child, Cst, Kind, Node},
    parse::Span,
    RitoType, Spanned,
};

use Diagnostic::*;

struct ListIter<'a, 'b, 'c> {
    ctx: &'a mut Builder<'b>,
    children: std::slice::Iter<'c, Child>,
    cst: &'c Cst,
    span: Span,
    type_span: Option<Span>,
    count: u8,
}

impl<'c> ListIter<'_, '_, 'c> {
    fn next_value(&mut self, expected: PropertyKind) -> Option<Result<Value, MaybeSpanDiag>> {
        for child in self.children.by_ref() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            if node.kind == Kind::Comment {
                continue;
            }
            self.count += 1;
            self.span = node.span;
            return Some(
                self.ctx
                    .resolve_numeric(node, expected, self.type_span)
                    .map_err(MaybeSpanDiag::from),
            );
        }
        None
    }

    fn expect_next(
        &mut self,
        expected_kind: PropertyKind,
        expected: ListLike,
    ) -> Result<Value, MaybeSpanDiag> {
        match self.next_value(expected_kind) {
            Some(v) => v,
            None => Err(NotEnoughItems {
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
            *slot = match self.expect_next(PropertyKind::F32, expected)? {
                Value::F32(v) => v.value,
                other => {
                    return Err(TypeMismatch {
                        span: other.span(),
                        expected: RitoType::simple(PropertyKind::F32),
                        expected_span: self.type_span,
                        got: RitoTypeOrVirtual::RitoType(RitoType::simple(other.kind())),
                    }
                    .into())
                }
            };
        }
        Ok(out)
    }

    fn read_u8s<const N: usize>(&mut self, expected: ListLike) -> Result<[u8; N], MaybeSpanDiag> {
        let mut out = [0u8; N];
        for slot in &mut out {
            *slot = match self.expect_next(PropertyKind::U8, expected)? {
                Value::U8(v) => v.value,
                other => {
                    return Err(TypeMismatch {
                        span: other.span(),
                        expected: RitoType::simple(PropertyKind::U8),
                        expected_span: self.type_span,
                        got: RitoTypeOrVirtual::RitoType(RitoType::simple(other.kind())),
                    }
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
        expected: PropertyKind,
        hint_span: Option<Span>,
    ) -> Result<Value, Diagnostic> {
        self.resolve_value(node, Some(RitoType::simple(expected)), hint_span)
    }

    pub(crate) fn resolve_listlike_fallable(
        &mut self,
        block: &Node,
        kind: PropertyKind,
        type_span: Option<Span>,
    ) -> Result<Value, MaybeSpanDiag> {
        let cst = self.cst();
        let span = block.span;
        let mut items = ListIter {
            ctx: self,
            children: block.children.get(cst).iter(),
            cst,
            span,
            type_span,
            count: 0,
        };

        let value = match kind {
            PropertyKind::Vector2 => {
                let [x, y] = items.read_floats::<2>(ListLike::Vec2)?;
                Value::Vector2(values::Vector2::new_with_meta([x, y].into(), span))
            }
            PropertyKind::Vector3 => {
                let [x, y, z] = items.read_floats::<3>(ListLike::Vec3)?;
                Value::Vector3(values::Vector3::new_with_meta([x, y, z].into(), span))
            }
            PropertyKind::Vector4 => {
                let [x, y, z, w] = items.read_floats::<4>(ListLike::Vec4)?;
                Value::Vector4(values::Vector4::new_with_meta([x, y, z, w].into(), span))
            }
            PropertyKind::Color => {
                let [r, g, b, a] = items.read_u8s::<4>(ListLike::Color)?;
                Value::Color(Spanned::new(span, ltk_primitives::Color { r, g, b, a }))
            }
            PropertyKind::Matrix44 => {
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
                Value::Matrix44(values::Matrix44::new_with_meta(mat, span))
            }
            _ => unreachable!("resolve_listlike called with a non-listlike kind"),
        };

        let expected = match kind {
            PropertyKind::Vector2 => ListLike::Vec2,
            PropertyKind::Vector3 => ListLike::Vec3,
            PropertyKind::Vector4 => ListLike::Vec4,
            PropertyKind::Color => ListLike::Color,
            PropertyKind::Matrix44 => ListLike::Mat44,
            _ => unreachable!(),
        };
        if let Some(extra) = items.next_value(PropertyKind::F32) {
            let extra = extra?;
            let count = 1 + items.children.count();
            return Err(TooManyItems {
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
        kind: PropertyKind,
        type_span: Option<Span>,
    ) -> Value {
        match self.resolve_listlike_fallable(block, kind, type_span) {
            Ok(value) => value,

            Err(e) => {
                self.push(e.fallback(block.span));
                Value::Unresolved {
                    kind,
                    span: block.span,
                }
            }
        }
    }
}
