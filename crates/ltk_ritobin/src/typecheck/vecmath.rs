use std::vec::Drain;

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

fn get_next(
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<PropertyValueEnum<Span>, Diagnostic> {
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
}

fn read_floats<const N: usize>(
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<[f32; N], MaybeSpanDiag> {
    let mut out = [0.0f32; N];
    for slot in &mut out {
        *slot = resolve_f32(get_next(span, items)?)?;
    }
    Ok(out)
}

fn read_u8s<const N: usize>(
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<[u8; N], MaybeSpanDiag> {
    let mut out = [0u8; N];
    for slot in &mut out {
        *slot = resolve_u8(get_next(span, items)?)?;
    }
    Ok(out)
}

fn inject_vec2(
    v: &mut values::Vector2<Span>,
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<ColorOrVec, MaybeSpanDiag> {
    v.value = read_floats::<2>(span, items)?.into();
    Ok(ColorOrVec::Vec2)
}

fn inject_vec3(
    v: &mut values::Vector3<Span>,
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<ColorOrVec, MaybeSpanDiag> {
    v.value = read_floats::<3>(span, items)?.into();
    Ok(ColorOrVec::Vec3)
}

fn inject_vec4(
    v: &mut values::Vector4<Span>,
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<ColorOrVec, MaybeSpanDiag> {
    v.value = read_floats::<4>(span, items)?.into();
    Ok(ColorOrVec::Vec4)
}

fn inject_color(
    v: &mut values::Color<Span>,
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<ColorOrVec, MaybeSpanDiag> {
    let [r, g, b, a] = read_u8s(span, items)?;
    let values::Color { value: color, .. } = v;
    color.r = r;
    color.g = g;
    color.b = b;
    color.a = a;
    Ok(ColorOrVec::Color)
}

fn inject_mat44(
    v: &mut values::Matrix44<Span>,
    span: &mut Span,
    items: &mut Drain<'_, IrListItem>,
) -> Result<ColorOrVec, MaybeSpanDiag> {
    let values::Matrix44 { value: mat, .. } = v;
    mat.x_axis = read_floats::<4>(span, items)?.into();
    mat.y_axis = read_floats::<4>(span, items)?.into();
    mat.z_axis = read_floats::<4>(span, items)?.into();
    mat.w_axis = read_floats::<4>(span, items)?.into();
    *mat = mat.transpose();
    Ok(ColorOrVec::Mat44)
}

pub(crate) fn populate_vec_or_color(
    target: &mut IrItem,
    items: &mut Vec<IrListItem>,
) -> Result<(), MaybeSpanDiag> {
    use PropertyValueEnum as V;

    let mut items = items.drain(..);
    let mut span = *target.value().meta(); // TODO: is this the right span to start with?

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
