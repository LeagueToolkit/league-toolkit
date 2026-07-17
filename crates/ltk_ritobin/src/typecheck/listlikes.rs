use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};

use crate::{
    parse::Span,
    typecheck::{
        diagnostics::{self, Diagnostic, ListLike, MaybeSpanDiag, RitoTypeOrVirtual},
        ir::{IrItem, IrListItem},
    },
    RitoType,
};

use diagnostics::Diagnostic::*;

fn resolve_f32(n: PropertyValueEnum<Span>) -> Result<f32, MaybeSpanDiag> {
    match n {
        PropertyValueEnum::F32(values::F32 { value: n, .. }) => Ok(n),
        _ => Err(TypeMismatch {
            span: *n.meta(),
            expected: RitoType::simple(PropertyKind::F32),
            expected_span: None, // TODO: would be nice
            got: RitoTypeOrVirtual::RitoType(RitoType::simple(n.kind())),
        }
        .into()),
    }
}

fn resolve_u8(n: PropertyValueEnum<Span>) -> Result<u8, MaybeSpanDiag> {
    match n {
        PropertyValueEnum::U8(values::U8 { value: n, .. }) => Ok(n),
        _ => Err(TypeMismatch {
            span: *n.meta(),
            expected: RitoType::simple(PropertyKind::U8),
            expected_span: None, // TODO: would be nice
            got: RitoTypeOrVirtual::RitoType(RitoType::simple(n.kind())),
        }
        .into()),
    }
}

struct ListIter<I: Iterator<Item = IrListItem>> {
    items: I,
    span: Span,
    count: u8,
}

impl<I: Iterator<Item = IrListItem>> ListIter<I> {
    fn new(span: Span, items: I) -> Self {
        Self {
            items,
            span,
            count: 0,
        }
    }

    fn next(&mut self) -> Option<IrListItem> {
        let item = self.items.next();
        if item.is_some() {
            self.count += 1;
        }
        item
    }

    fn into_inner(self) -> I {
        self.items
    }

    fn expect_next(&mut self, expected: ListLike) -> Result<PropertyValueEnum<Span>, Diagnostic> {
        let item = self
            .next()
            .ok_or(NotEnoughItems {
                span: self.span,
                got: self.count,
                expected,
            })?
            .0;
        self.span = *item.meta();
        Ok(item)
    }
    fn read_floats<const N: usize>(
        &mut self,
        expected: ListLike,
    ) -> Result<[f32; N], MaybeSpanDiag> {
        let mut out = [0.0f32; N];
        for slot in &mut out {
            *slot = resolve_f32(self.expect_next(expected)?)?;
        }
        Ok(out)
    }
    fn read_u8s<const N: usize>(&mut self, expected: ListLike) -> Result<[u8; N], MaybeSpanDiag> {
        let mut out = [0u8; N];
        for slot in &mut out {
            *slot = resolve_u8(self.expect_next(expected)?)?;
        }
        Ok(out)
    }

    fn inject_vec2(
        &mut self,
        v: &mut values::Vector2<Span>,
        expected: ListLike,
    ) -> Result<ListLike, MaybeSpanDiag> {
        let expect = ListLike::Vec2;
        v.value = self.read_floats::<2>(expected)?.into();
        Ok(expect)
    }

    fn inject_vec3(
        &mut self,
        v: &mut values::Vector3<Span>,
        expected: ListLike,
    ) -> Result<ListLike, MaybeSpanDiag> {
        let expect = ListLike::Vec3;
        v.value = self.read_floats::<3>(expected)?.into();
        Ok(expect)
    }

    fn inject_vec4(
        &mut self,
        v: &mut values::Vector4<Span>,
        expected: ListLike,
    ) -> Result<ListLike, MaybeSpanDiag> {
        let expect = ListLike::Vec4;
        v.value = self.read_floats::<4>(expected)?.into();
        Ok(expect)
    }

    fn inject_color(
        &mut self,
        v: &mut values::Color<Span>,
        expected: ListLike,
    ) -> Result<ListLike, MaybeSpanDiag> {
        let expect = ListLike::Color;
        let [r, g, b, a] = self.read_u8s(expected)?;
        let values::Color { value: color, .. } = v;
        color.r = r;
        color.g = g;
        color.b = b;
        color.a = a;
        Ok(expect)
    }

    fn inject_mat44(
        &mut self,
        v: &mut values::Matrix44<Span>,
        expected: ListLike,
    ) -> Result<ListLike, MaybeSpanDiag> {
        let expect = ListLike::Mat44;
        let values::Matrix44 { value: mat, .. } = v;
        mat.x_axis = self.read_floats::<4>(expected)?.into();
        mat.y_axis = self.read_floats::<4>(expected)?.into();
        mat.z_axis = self.read_floats::<4>(expected)?.into();
        mat.w_axis = self.read_floats::<4>(expected)?.into();
        *mat = mat.transpose();
        Ok(expect)
    }
}

/// Try populate a listlike type (vec, mtx44, rgba, option[listlike])
pub(crate) fn try_populate_listlike(
    target: &mut IrItem,
    items: &mut Vec<IrListItem>,
) -> Result<(), MaybeSpanDiag> {
    use PropertyValueEnum as V;

    // empty options look like empty lists, return early so we don't complain about missing items
    if items.is_empty() && matches!(target.value(), V::Optional(_)) {
        return Ok(());
    }

    // TODO: is this the right span to start with?
    let mut items = ListIter::new(*target.value().meta(), items.drain(..));

    let mut inject =
        |target: &mut PropertyValueEnum<Span>| -> Result<Option<ListLike>, MaybeSpanDiag> {
            Ok(Some(match target {
                V::Vector2(v) => items.inject_vec2(v, ListLike::Vec2)?,
                V::Vector3(v) => items.inject_vec3(v, ListLike::Vec3)?,
                V::Vector4(v) => items.inject_vec4(v, ListLike::Vec4)?,
                V::Color(v) => items.inject_color(v, ListLike::Color)?,
                V::Matrix44(v) => items.inject_mat44(v, ListLike::Mat44)?,
                V::Optional(opt) => match opt {
                    values::Optional::Vector2 { value, .. } => {
                        items.inject_vec2(value.get_or_insert_default(), ListLike::Vec2)?
                    }
                    values::Optional::Vector3 { value, .. } => {
                        items.inject_vec3(value.get_or_insert_default(), ListLike::Vec3)?
                    }
                    values::Optional::Vector4 { value, .. } => {
                        items.inject_vec4(value.get_or_insert_default(), ListLike::Vec4)?
                    }
                    values::Optional::Color { value, .. } => {
                        items.inject_color(value.get_or_insert_default(), ListLike::Color)?
                    }
                    values::Optional::Matrix44 { value, .. } => {
                        items.inject_mat44(value.get_or_insert_default(), ListLike::Mat44)?
                    }
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }))
        };

    let Some(expected) = inject(target.value_mut())? else {
        // we weren't a listlike
        return Ok(());
    };

    if let Some(extra) = items.next() {
        let count = 1 + items.into_inner().count();
        return Err(TooManyItems {
            span: *extra.0.meta(),
            extra: count as _,
            expected,
        }
        .into());
    }
    Ok(())
}
