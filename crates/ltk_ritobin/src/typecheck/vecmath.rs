use std::vec::Drain;

use glam::Vec4;
use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};

use crate::{
    parse::Span,
    typecheck::{
        diagnostics::{self, ColorOrVec, Diagnostic, MaybeSpanDiag, RitoTypeOrVirtual},
        ir::{IrItem, IrListItem},
    },
    RitoType,
};

use diagnostics::Diagnostic::*;

pub(crate) fn populate_vec_or_color(
    target: &mut IrItem,
    items: &mut Vec<IrListItem>,
) -> Result<(), MaybeSpanDiag> {
    let resolve_f32 = |n: PropertyValueEnum<Span>| -> Result<f32, MaybeSpanDiag> {
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
    };
    let resolve_u8 = |n: PropertyValueEnum<Span>| -> Result<u8, MaybeSpanDiag> {
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
    };

    let mut items = items.drain(..);
    let get_next = |span: &mut Span, items: &mut Drain<'_, IrListItem>| -> Result<_, Diagnostic> {
        let item = items
            .next()
            .ok_or(NotEnoughItems {
                span: *span,
                got: 0,
                expected: ColorOrVec::Vec2,
            })?
            .0;
        *span = *item.meta();
        Ok(item)
    };

    use PropertyValueEnum as V;
    let mut span = *target.value().meta(); // TODO: is this the right span to start with?

    let inject_vec2 = |v: &mut values::Vector2<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector2 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec2)
    };
    let inject_vec3 = |v: &mut values::Vector3<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector3 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        vec.z = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec3)
    };
    let inject_vec4 = |v: &mut values::Vector4<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector4 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        vec.z = resolve_f32(get_next(span, items)?)?;
        vec.w = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec4)
    };
    let inject_color = |v: &mut values::Color<Span>,
                        span: &mut Span,
                        items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Color { value: color, .. } = v;
        color.r = resolve_u8(get_next(span, items)?)?;
        color.g = resolve_u8(get_next(span, items)?)?;
        color.b = resolve_u8(get_next(span, items)?)?;
        color.a = resolve_u8(get_next(span, items)?)?;
        Ok(ColorOrVec::Color)
    };
    let inject_mat44 = |v: &mut values::Matrix44<Span>,
                        span: &mut Span,
                        items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Matrix44 { value: mat, .. } = v;
        mat.x_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.y_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.z_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.w_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        *mat = mat.transpose();
        Ok(ColorOrVec::Mat44)
    };

    let mut inject =
        |target: &mut PropertyValueEnum<Span>| -> Result<Option<ColorOrVec>, MaybeSpanDiag> {
            Ok(Some(match target {
                V::Vector2(v) => inject_vec2(v, &mut span, &mut items)?,
                V::Vector3(v) => inject_vec3(v, &mut span, &mut items)?,
                V::Vector4(v) => inject_vec4(v, &mut span, &mut items)?,
                V::Color(v) => inject_color(v, &mut span, &mut items)?,
                V::Matrix44(v) => inject_mat44(v, &mut span, &mut items)?,
                V::Optional(opt) => match opt {
                    values::Optional::Vector2 { value, .. } => {
                        inject_vec2(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Vector3 { value, .. } => {
                        inject_vec3(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Vector4 { value, .. } => {
                        inject_vec4(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Color { value, .. } => {
                        inject_color(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Matrix44 { value, .. } => {
                        inject_mat44(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }))
        };

    let expected = inject(target.value_mut())?.ok_or(CustomSpan(
        "non-empty list queue with non color/vec type receiver",
        span,
    ))?;

    if let Some(extra) = items.next() {
        let count = 1 + items.count();
        return Err(TooManyItems {
            span: *extra.0.meta(),
            extra: count as _,
            expected,
        }
        .into());
    }
    Ok(())
}
