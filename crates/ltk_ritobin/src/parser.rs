//! Nom parser for ritobin text format with span tracking for error reporting.

// Nom-style parsers use elided lifetimes extensively
#![allow(clippy::type_complexity)]

use glam::{Mat4, Vec2, Vec3, Vec4};
use indexmap::IndexMap;
use ltk_hash::fnv1a::hash_lower;
use ltk_meta::{
    value::{
        BitBoolValue, BoolValue, ColorValue, ContainerValue, EmbeddedValue, F32Value, HashValue,
        I16Value, I32Value, I64Value, I8Value, MapValue, Matrix44Value, NoneValue, ObjectLinkValue,
        OptionalValue, PropertyValueEnum, PropertyValueUnsafeEq, StringValue, StructValue,
        U16Value, U32Value, U64Value, U8Value, UnorderedContainerValue, Vector2Value, Vector3Value,
        Vector4Value, WadChunkLinkValue,
    },
    BinProperty, BinPropertyKind, BinTree, BinTreeObject,
};
use ltk_primitives::Color;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until, take_while, take_while1},
    character::complete::{char, hex_digit1, multispace1, one_of},
    combinator::{map, map_res, opt, recognize, value},
    error::{ErrorKind, FromExternalError, ParseError as NomParseError},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
    Err as NomErr, IResult,
};
use nom_locate::LocatedSpan;

use crate::{
    error::ParseError,
    types::{type_name_to_kind, RitobinType},
};

// ============================================================================
// Span Types and Custom Error
// ============================================================================

/// Input type that tracks position in the source.
pub type Span<'a> = LocatedSpan<&'a str>;

/// Custom error type that preserves span information.
#[derive(Debug, Clone)]
pub struct SpannedError<'a> {
    pub span: Span<'a>,
    pub kind: SpannedErrorKind,
}

#[derive(Debug, Clone)]
pub enum SpannedErrorKind {
    Nom(ErrorKind),
    Expected(&'static str),
    UnknownType(String),
    InvalidNumber(String),
    InvalidHex(String),
    UnclosedString,
    UnclosedBlock,
    Context(&'static str),
}

impl<'a> NomParseError<Span<'a>> for SpannedError<'a> {
    fn from_error_kind(input: Span<'a>, kind: ErrorKind) -> Self {
        SpannedError {
            span: input,
            kind: SpannedErrorKind::Nom(kind),
        }
    }

    fn append(_input: Span<'a>, _kind: ErrorKind, other: Self) -> Self {
        other
    }
}

impl<'a, E> FromExternalError<Span<'a>, E> for SpannedError<'a> {
    fn from_external_error(input: Span<'a>, kind: ErrorKind, _e: E) -> Self {
        SpannedError {
            span: input,
            kind: SpannedErrorKind::Nom(kind),
        }
    }
}

impl<'a> SpannedError<'a> {
    pub fn expected(span: Span<'a>, what: &'static str) -> Self {
        SpannedError {
            span,
            kind: SpannedErrorKind::Expected(what),
        }
    }

    pub fn unknown_type(span: Span<'a>, type_name: String) -> Self {
        SpannedError {
            span,
            kind: SpannedErrorKind::UnknownType(type_name),
        }
    }

    pub fn to_parse_error(&self, src: &str) -> ParseError {
        let offset = self.span.location_offset();
        let len = self.span.fragment().len().max(1);

        match &self.kind {
            SpannedErrorKind::Nom(kind) => ParseError::ParseErrorAt {
                message: format!("{:?}", kind),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::Expected(what) => ParseError::Expected {
                expected: (*what).to_string(),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::UnknownType(name) => ParseError::UnknownType {
                type_name: name.clone(),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::InvalidNumber(val) => ParseError::InvalidNumber {
                value: val.clone(),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::InvalidHex(val) => ParseError::InvalidHex {
                value: val.clone(),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::UnclosedString => ParseError::UnclosedString {
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::UnclosedBlock => ParseError::UnclosedBlock {
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
            SpannedErrorKind::Context(ctx) => ParseError::ParseErrorAt {
                message: (*ctx).to_string(),
                src: src.to_string(),
                span: miette::SourceSpan::new(offset.into(), len),
            },
        }
    }
}

type ParseResult<'a, T> = IResult<Span<'a>, T, SpannedError<'a>>;

// ============================================================================
// Basic Parsers
// ============================================================================

/// Parse whitespace and comments (lines starting with #, except at the very beginning).
fn ws(input: Span) -> ParseResult<()> {
    value(
        (),
        many0(alt((
            value((), multispace1),
            value(
                (),
                pair(char('#'), alt((take_until("\n"), take_while(|_| true)))),
            ),
        ))),
    )(input)
}

/// Parse an identifier (alphanumeric + underscore, starting with letter or underscore).
fn identifier(input: Span) -> ParseResult<Span> {
    preceded(ws, take_while1(|c: char| c.is_alphanumeric() || c == '_'))(input)
}

/// Parse a word that can include various characters found in paths/identifiers.
fn word(input: Span) -> ParseResult<Span> {
    preceded(
        ws,
        take_while1(|c: char| {
            c.is_alphanumeric() || c == '_' || c == '+' || c == '-' || c == '.' || c == '/'
        }),
    )(input)
}

/// Parse a quoted string with escape sequences.
fn quoted_string(input: Span) -> ParseResult<String> {
    preceded(
        ws,
        alt((
            delimited(
                char('"'),
                map(
                    many0(alt((
                        map(is_not("\\\""), |s: Span| s.fragment().to_string()),
                        map(preceded(char('\\'), one_of("nrt\\\"'")), |c| match c {
                            'n' => "\n".to_string(),
                            'r' => "\r".to_string(),
                            't' => "\t".to_string(),
                            '\\' => "\\".to_string(),
                            '"' => "\"".to_string(),
                            '\'' => "'".to_string(),
                            _ => c.to_string(),
                        }),
                    ))),
                    |parts| parts.join(""),
                ),
                char('"'),
            ),
            delimited(
                char('\''),
                map(
                    many0(alt((
                        map(is_not("\\'"), |s: Span| s.fragment().to_string()),
                        map(preceded(char('\\'), one_of("nrt\\\"'")), |c| match c {
                            'n' => "\n".to_string(),
                            'r' => "\r".to_string(),
                            't' => "\t".to_string(),
                            '\\' => "\\".to_string(),
                            '"' => "\"".to_string(),
                            '\'' => "'".to_string(),
                            _ => c.to_string(),
                        }),
                    ))),
                    |parts| parts.join(""),
                ),
                char('\''),
            ),
        )),
    )(input)
}

/// Parse a hex u32 (0x12345678 or decimal).
fn hex_u32(input: Span) -> ParseResult<u32> {
    preceded(
        ws,
        alt((
            map_res(
                preceded(alt((tag("0x"), tag("0X"))), hex_digit1),
                |s: Span| u32::from_str_radix(s.fragment(), 16),
            ),
            map_res(
                recognize(pair(
                    opt(char('-')),
                    take_while1(|c: char| c.is_ascii_digit()),
                )),
                |s: Span| s.fragment().parse::<u32>(),
            ),
        )),
    )(input)
}

/// Parse a hex u64 (0x123456789abcdef0 or decimal).
fn hex_u64(input: Span) -> ParseResult<u64> {
    preceded(
        ws,
        alt((
            map_res(
                preceded(alt((tag("0x"), tag("0X"))), hex_digit1),
                |s: Span| u64::from_str_radix(s.fragment(), 16),
            ),
            map_res(take_while1(|c: char| c.is_ascii_digit()), |s: Span| {
                s.fragment().parse::<u64>()
            }),
        )),
    )(input)
}

/// Parse a boolean.
fn parse_bool(input: Span) -> ParseResult<bool> {
    preceded(
        ws,
        alt((value(true, tag("true")), value(false, tag("false")))),
    )(input)
}

/// Parse an integer number.
fn parse_int<T: std::str::FromStr>(input: Span) -> ParseResult<T> {
    preceded(
        ws,
        map_res(
            recognize(pair(
                opt(char('-')),
                take_while1(|c: char| c.is_ascii_digit()),
            )),
            |s: Span| s.fragment().parse::<T>(),
        ),
    )(input)
}

/// Parse a float number.
fn parse_float(input: Span) -> ParseResult<f32> {
    preceded(
        ws,
        map_res(
            recognize(tuple((
                opt(char('-')),
                take_while1(|c: char| c.is_ascii_digit() || c == '.'),
                opt(pair(
                    one_of("eE"),
                    pair(opt(one_of("+-")), take_while1(|c: char| c.is_ascii_digit())),
                )),
            ))),
            |s: Span| s.fragment().parse::<f32>(),
        ),
    )(input)
}

// ============================================================================
// Type Parsers
// ============================================================================

/// Parse a type name and return the BinPropertyKind.
fn parse_type_name(input: Span) -> ParseResult<BinPropertyKind> {
    let (input, type_span) = word(input)?;
    match type_name_to_kind(type_span.fragment()) {
        Some(kind) => Ok((input, kind)),
        None => Err(NomErr::Failure(SpannedError::unknown_type(
            type_span,
            type_span.fragment().to_string(),
        ))),
    }
}

/// Parse container type parameters: `\[type\]` or `\[key,value\]`.
fn parse_container_type_params(
    input: Span,
) -> ParseResult<(BinPropertyKind, Option<BinPropertyKind>)> {
    preceded(
        ws,
        delimited(
            char('['),
            alt((
                // map[key,value]
                map(
                    tuple((
                        parse_type_name,
                        preceded(tuple((ws, char(','), ws)), parse_type_name),
                    )),
                    |(k, v)| (k, Some(v)),
                ),
                // list[type] or option[type]
                map(parse_type_name, |t| (t, None)),
            )),
            preceded(ws, char(']')),
        ),
    )(input)
}

/// Parse a full type specification including container parameters.
fn parse_type(input: Span) -> ParseResult<RitobinType> {
    let (input, kind) = parse_type_name(input)?;

    if kind.is_container() || kind == BinPropertyKind::Optional {
        let (input, (inner, value_kind)) = parse_container_type_params(input)?;
        if kind == BinPropertyKind::Map {
            Ok((
                input,
                RitobinType::map(inner, value_kind.unwrap_or(BinPropertyKind::None)),
            ))
        } else {
            Ok((input, RitobinType::container(kind, inner)))
        }
    } else {
        Ok((input, RitobinType::simple(kind)))
    }
}

// ============================================================================
// Value Parsers
// ============================================================================

/// Parse a vec2: { x, y }
fn parse_vec2(input: Span) -> ParseResult<Vec2> {
    delimited(
        preceded(ws, char('{')),
        map(
            tuple((
                parse_float,
                preceded(tuple((ws, char(','), ws)), parse_float),
            )),
            |(x, y)| Vec2::new(x, y),
        ),
        preceded(ws, char('}')),
    )(input)
}

/// Parse a vec3: { x, y, z }
fn parse_vec3(input: Span) -> ParseResult<Vec3> {
    delimited(
        preceded(ws, char('{')),
        map(
            tuple((
                parse_float,
                preceded(tuple((ws, char(','), ws)), parse_float),
                preceded(tuple((ws, char(','), ws)), parse_float),
            )),
            |(x, y, z)| Vec3::new(x, y, z),
        ),
        preceded(ws, char('}')),
    )(input)
}

/// Parse a vec4: { x, y, z, w }
fn parse_vec4(input: Span) -> ParseResult<Vec4> {
    delimited(
        preceded(ws, char('{')),
        map(
            tuple((
                parse_float,
                preceded(tuple((ws, char(','), ws)), parse_float),
                preceded(tuple((ws, char(','), ws)), parse_float),
                preceded(tuple((ws, char(','), ws)), parse_float),
            )),
            |(x, y, z, w)| Vec4::new(x, y, z, w),
        ),
        preceded(ws, char('}')),
    )(input)
}

/// Parse a mtx44: { 16 floats }
fn parse_mtx44(input: Span) -> ParseResult<Mat4> {
    let (input, _) = preceded(ws, char('{'))(input)?;

    let mut values = [0.0f32; 16];
    let mut remaining = input;

    for (i, val) in values.iter_mut().enumerate() {
        let (r, _) = ws(remaining)?;
        let (r, v) = parse_float(r)?;
        *val = v;

        // Handle optional comma or whitespace between values
        let (r, _) = ws(r)?;
        let (r, _) = opt(char(','))(r)?;
        remaining = r;

        if i < 15 {
            let (r, _) = ws(remaining)?;
            remaining = r;
        }
    }

    let (remaining, _) = preceded(ws, char('}'))(remaining)?;

    Ok((remaining, Mat4::from_cols_array(&values)))
}

/// Parse rgba: { r, g, b, a }
fn parse_rgba(input: Span) -> ParseResult<Color<u8>> {
    delimited(
        preceded(ws, char('{')),
        map(
            tuple((
                parse_int::<u8>,
                preceded(tuple((ws, char(','), ws)), parse_int::<u8>),
                preceded(tuple((ws, char(','), ws)), parse_int::<u8>),
                preceded(tuple((ws, char(','), ws)), parse_int::<u8>),
            )),
            |(r, g, b, a)| Color::new(r, g, b, a),
        ),
        preceded(ws, char('}')),
    )(input)
}

/// Parse a hash value (hex or quoted string that gets hashed).
fn parse_hash_value(input: Span) -> ParseResult<u32> {
    preceded(ws, alt((map(quoted_string, |s| hash_lower(&s)), hex_u32)))(input)
}

/// Parse a file hash (u64).
fn parse_file_hash(input: Span) -> ParseResult<u64> {
    preceded(
        ws,
        alt((
            map(quoted_string, |s| {
                xxhash_rust::xxh64::xxh64(s.to_lowercase().as_bytes(), 0)
            }),
            hex_u64,
        )),
    )(input)
}

/// Parse a link value (hash or quoted string).
fn parse_link_value(input: Span) -> ParseResult<u32> {
    preceded(ws, alt((map(quoted_string, |s| hash_lower(&s)), hex_u32)))(input)
}

/// Parse items in a list/container.
fn parse_list_items(
    input: Span,
    item_kind: BinPropertyKind,
) -> ParseResult<Vec<PropertyValueEnum>> {
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = ws(input)?;

    let mut items = Vec::new();
    let mut remaining = input;

    loop {
        let (r, _) = ws(remaining)?;

        // Check for closing brace
        if let Ok((r, _)) = char::<Span, SpannedError>('}')(r) {
            return Ok((r, items));
        }

        // Parse an item
        let (r, item) = parse_value_for_kind(r, item_kind)?;
        items.push(item);

        let (r, _) = ws(r)?;
        // Optional comma or newline separator
        let (r, _) = opt(char(','))(r)?;
        remaining = r;
    }
}

/// Parse map entries.
fn parse_map_entries(
    input: Span,
    key_kind: BinPropertyKind,
    value_kind: BinPropertyKind,
) -> ParseResult<IndexMap<PropertyValueUnsafeEq, PropertyValueEnum>> {
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = ws(input)?;

    let mut entries = IndexMap::new();
    let mut remaining = input;

    loop {
        let (r, _) = ws(remaining)?;

        // Check for closing brace
        if let Ok((r, _)) = char::<Span, SpannedError>('}')(r) {
            return Ok((r, entries));
        }

        // Parse key = value
        let (r, key) = parse_value_for_kind(r, key_kind)?;
        let (r, _) = preceded(ws, char('='))(r)?;
        let (r, value) = parse_value_for_kind(r, value_kind)?;

        entries.insert(PropertyValueUnsafeEq(key), value);

        let (r, _) = ws(r)?;
        let (r, _) = opt(char(','))(r)?;
        remaining = r;
    }
}

/// Parse optional value.
fn parse_optional_value(input: Span, inner_kind: BinPropertyKind) -> ParseResult<OptionalValue> {
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = ws(input)?;

    // Check for empty optional
    if let Ok((input, _)) = char::<Span, SpannedError>('}')(input) {
        return Ok((
            input,
            OptionalValue {
                kind: inner_kind,
                value: None,
            },
        ));
    }

    // Parse the inner value
    let (input, value) = parse_value_for_kind(input, inner_kind)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;

    Ok((
        input,
        OptionalValue {
            kind: inner_kind,
            value: Some(Box::new(value)),
        },
    ))
}

/// Parse struct/embed fields.
fn parse_struct_fields(input: Span) -> ParseResult<IndexMap<u32, BinProperty>> {
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = ws(input)?;

    let mut properties = IndexMap::new();
    let mut remaining = input;

    loop {
        let (r, _) = ws(remaining)?;

        // Check for closing brace
        if let Ok((r, _)) = char::<Span, SpannedError>('}')(r) {
            return Ok((r, properties));
        }

        // Parse field: name: type = value
        let (r, field) = parse_field(r)?;
        properties.insert(field.name_hash, field);

        let (r, _) = ws(r)?;
        let (r, _) = opt(char(','))(r)?;
        remaining = r;
    }
}

/// Parse a single field: name: type = value
fn parse_field(input: Span) -> ParseResult<BinProperty> {
    let (input, _) = ws(input)?;
    let (input, name_span) = word(input)?;
    let name_str = *name_span.fragment();

    // Determine hash from name
    let name_hash = if name_str.starts_with("0x") || name_str.starts_with("0X") {
        u32::from_str_radix(&name_str[2..], 16).unwrap_or(0)
    } else {
        hash_lower(name_str)
    };

    let (input, _) = preceded(ws, char(':'))(input)?;
    let (input, ty) = parse_type(input)?;
    let (input, _) = preceded(ws, char('='))(input)?;
    let (input, value) = parse_value_for_type(input, &ty)?;

    Ok((input, BinProperty { name_hash, value }))
}

/// Parse a pointer value (null or name { fields }).
fn parse_pointer_value(input: Span) -> ParseResult<StructValue> {
    let (input, _) = ws(input)?;

    // Check for null
    if let Ok((input, _)) = tag::<&str, Span, SpannedError>("null")(input) {
        return Ok((
            input,
            StructValue {
                class_hash: 0,
                properties: IndexMap::new(),
            },
        ));
    }

    // Parse class name
    let (input, class_span) = word(input)?;
    let class_str = *class_span.fragment();
    let class_hash = if class_str.starts_with("0x") || class_str.starts_with("0X") {
        u32::from_str_radix(&class_str[2..], 16).unwrap_or(0)
    } else {
        hash_lower(class_str)
    };

    // Check for empty struct
    let (input, _) = ws(input)?;
    if let Ok((input, _)) = tag::<&str, Span, SpannedError>("{}")(input) {
        return Ok((
            input,
            StructValue {
                class_hash,
                properties: IndexMap::new(),
            },
        ));
    }

    // Parse fields
    let (input, properties) = parse_struct_fields(input)?;

    Ok((
        input,
        StructValue {
            class_hash,
            properties,
        },
    ))
}

/// Parse an embed value (name { fields }).
fn parse_embed_value(input: Span) -> ParseResult<EmbeddedValue> {
    let (input, struct_val) = parse_pointer_value(input)?;
    Ok((input, EmbeddedValue(struct_val)))
}

/// Parse a value given a BinPropertyKind.
fn parse_value_for_kind(input: Span, kind: BinPropertyKind) -> ParseResult<PropertyValueEnum> {
    match kind {
        BinPropertyKind::None => {
            let (input, _) = preceded(ws, tag("null"))(input)?;
            Ok((input, PropertyValueEnum::None(NoneValue)))
        }
        BinPropertyKind::Bool => {
            let (input, v) = parse_bool(input)?;
            Ok((input, PropertyValueEnum::Bool(BoolValue(v))))
        }
        BinPropertyKind::I8 => {
            let (input, v) = parse_int::<i8>(input)?;
            Ok((input, PropertyValueEnum::I8(I8Value(v))))
        }
        BinPropertyKind::U8 => {
            let (input, v) = parse_int::<u8>(input)?;
            Ok((input, PropertyValueEnum::U8(U8Value(v))))
        }
        BinPropertyKind::I16 => {
            let (input, v) = parse_int::<i16>(input)?;
            Ok((input, PropertyValueEnum::I16(I16Value(v))))
        }
        BinPropertyKind::U16 => {
            let (input, v) = parse_int::<u16>(input)?;
            Ok((input, PropertyValueEnum::U16(U16Value(v))))
        }
        BinPropertyKind::I32 => {
            let (input, v) = parse_int::<i32>(input)?;
            Ok((input, PropertyValueEnum::I32(I32Value(v))))
        }
        BinPropertyKind::U32 => {
            let (input, v) = hex_u32(input)?;
            Ok((input, PropertyValueEnum::U32(U32Value(v))))
        }
        BinPropertyKind::I64 => {
            let (input, v) = parse_int::<i64>(input)?;
            Ok((input, PropertyValueEnum::I64(I64Value(v))))
        }
        BinPropertyKind::U64 => {
            let (input, v) = hex_u64(input)?;
            Ok((input, PropertyValueEnum::U64(U64Value(v))))
        }
        BinPropertyKind::F32 => {
            let (input, v) = parse_float(input)?;
            Ok((input, PropertyValueEnum::F32(F32Value(v))))
        }
        BinPropertyKind::Vector2 => {
            let (input, v) = parse_vec2(input)?;
            Ok((input, PropertyValueEnum::Vector2(Vector2Value(v))))
        }
        BinPropertyKind::Vector3 => {
            let (input, v) = parse_vec3(input)?;
            Ok((input, PropertyValueEnum::Vector3(Vector3Value(v))))
        }
        BinPropertyKind::Vector4 => {
            let (input, v) = parse_vec4(input)?;
            Ok((input, PropertyValueEnum::Vector4(Vector4Value(v))))
        }
        BinPropertyKind::Matrix44 => {
            let (input, v) = parse_mtx44(input)?;
            Ok((input, PropertyValueEnum::Matrix44(Matrix44Value(v))))
        }
        BinPropertyKind::Color => {
            let (input, v) = parse_rgba(input)?;
            Ok((input, PropertyValueEnum::Color(ColorValue(v))))
        }
        BinPropertyKind::String => {
            let (input, v) = preceded(ws, quoted_string)(input)?;
            Ok((input, PropertyValueEnum::String(StringValue(v))))
        }
        BinPropertyKind::Hash => {
            let (input, v) = parse_hash_value(input)?;
            Ok((input, PropertyValueEnum::Hash(HashValue(v))))
        }
        BinPropertyKind::WadChunkLink => {
            let (input, v) = parse_file_hash(input)?;
            Ok((input, PropertyValueEnum::WadChunkLink(WadChunkLinkValue(v))))
        }
        BinPropertyKind::ObjectLink => {
            let (input, v) = parse_link_value(input)?;
            Ok((input, PropertyValueEnum::ObjectLink(ObjectLinkValue(v))))
        }
        BinPropertyKind::BitBool => {
            let (input, v) = parse_bool(input)?;
            Ok((input, PropertyValueEnum::BitBool(BitBoolValue(v))))
        }
        BinPropertyKind::Struct => {
            let (input, v) = parse_pointer_value(input)?;
            Ok((input, PropertyValueEnum::Struct(v)))
        }
        BinPropertyKind::Embedded => {
            let (input, v) = parse_embed_value(input)?;
            Ok((input, PropertyValueEnum::Embedded(v)))
        }
        // Container types need additional type info, handled separately
        BinPropertyKind::Container
        | BinPropertyKind::UnorderedContainer
        | BinPropertyKind::Optional
        | BinPropertyKind::Map => Err(NomErr::Failure(SpannedError::expected(
            input,
            "non-container type",
        ))),
    }
}

/// Parse a value given a full RitobinType.
fn parse_value_for_type<'a>(
    input: Span<'a>,
    ty: &RitobinType,
) -> ParseResult<'a, PropertyValueEnum> {
    match ty.kind {
        BinPropertyKind::Container => {
            let inner_kind = ty.inner_kind.unwrap_or(BinPropertyKind::None);
            let (input, items) = parse_list_items(input, inner_kind)?;
            Ok((
                input,
                PropertyValueEnum::Container(ContainerValue {
                    item_kind: inner_kind,
                    items,
                }),
            ))
        }
        BinPropertyKind::UnorderedContainer => {
            let inner_kind = ty.inner_kind.unwrap_or(BinPropertyKind::None);
            let (input, items) = parse_list_items(input, inner_kind)?;
            Ok((
                input,
                PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
                    item_kind: inner_kind,
                    items,
                })),
            ))
        }
        BinPropertyKind::Optional => {
            let inner_kind = ty.inner_kind.unwrap_or(BinPropertyKind::None);
            let (input, opt_val) = parse_optional_value(input, inner_kind)?;
            Ok((input, PropertyValueEnum::Optional(opt_val)))
        }
        BinPropertyKind::Map => {
            let key_kind = ty.inner_kind.unwrap_or(BinPropertyKind::Hash);
            let value_kind = ty.value_kind.unwrap_or(BinPropertyKind::None);
            let (input, entries) = parse_map_entries(input, key_kind, value_kind)?;
            Ok((
                input,
                PropertyValueEnum::Map(MapValue {
                    key_kind,
                    value_kind,
                    entries,
                }),
            ))
        }
        _ => parse_value_for_kind(input, ty.kind),
    }
}

// ============================================================================
// Top-Level Parsers
// ============================================================================

/// Parse a top-level entry: key: type = value
fn parse_entry(input: Span) -> ParseResult<(String, BinProperty)> {
    let (input, _) = ws(input)?;
    let (input, key) = identifier(input)?;
    let (input, _) = preceded(ws, char(':'))(input)?;
    let (input, ty) = parse_type(input)?;
    let (input, _) = preceded(ws, char('='))(input)?;
    let (input, value) = parse_value_for_type(input, &ty)?;

    let name_hash = hash_lower(key.fragment());

    Ok((
        input,
        (key.fragment().to_string(), BinProperty { name_hash, value }),
    ))
}

/// Parse the entire ritobin file.
fn parse_ritobin(input: Span) -> ParseResult<RitobinFile> {
    let (input, _) = ws(input)?;
    // The header comment is consumed by ws, but we should verify it's present
    // For now, we're lenient about the header

    let (input, entries) = many0(parse_entry)(input)?;
    let (input, _) = ws(input)?;

    let mut file = RitobinFile::new();
    for (key, prop) in entries {
        file.entries.insert(key, prop);
    }

    Ok((input, file))
}

// ============================================================================
// Public Types and API
// ============================================================================

/// A ritobin file representation (intermediate format before conversion to BinTree).
#[derive(Debug, Clone, Default)]
pub struct RitobinFile {
    pub entries: IndexMap<String, BinProperty>,
}

impl RitobinFile {
    pub fn new() -> Self {
        Self {
            entries: IndexMap::new(),
        }
    }

    /// Get the "type" field value as a string.
    pub fn file_type(&self) -> Option<&str> {
        self.entries.get("type").and_then(|p| {
            if let PropertyValueEnum::String(StringValue(s)) = &p.value {
                Some(s.as_str())
            } else {
                None
            }
        })
    }

    /// Get the "version" field as u32.
    pub fn version(&self) -> Option<u32> {
        self.entries.get("version").and_then(|p| {
            if let PropertyValueEnum::U32(U32Value(v)) = &p.value {
                Some(*v)
            } else {
                None
            }
        })
    }

    /// Get the "linked" dependencies list.
    pub fn linked(&self) -> Vec<String> {
        self.entries
            .get("linked")
            .and_then(|p| {
                if let PropertyValueEnum::Container(ContainerValue { items, .. }) = &p.value {
                    Some(
                        items
                            .iter()
                            .filter_map(|item| {
                                if let PropertyValueEnum::String(StringValue(s)) = item {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Get the "entries" map as BinTreeObjects.
    pub fn objects(&self) -> IndexMap<u32, BinTreeObject> {
        self.entries
            .get("entries")
            .and_then(|p| {
                if let PropertyValueEnum::Map(MapValue { entries, .. }) = &p.value {
                    Some(
                        entries
                            .iter()
                            .filter_map(|(key, value)| {
                                let path_hash = match &key.0 {
                                    PropertyValueEnum::Hash(HashValue(h)) => *h,
                                    _ => return None,
                                };

                                if let PropertyValueEnum::Embedded(EmbeddedValue(struct_val)) =
                                    value
                                {
                                    Some((
                                        path_hash,
                                        BinTreeObject {
                                            path_hash,
                                            class_hash: struct_val.class_hash,
                                            properties: struct_val.properties.clone(),
                                        },
                                    ))
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Convert to a BinTree.
    pub fn to_bin_tree(&self) -> BinTree {
        BinTree::new(self.objects().into_values(), self.linked())
    }
}

/// Parse ritobin text format.
pub fn parse(input: &str) -> Result<RitobinFile, ParseError> {
    let span = Span::new(input);
    match parse_ritobin(span) {
        Ok((remaining, file)) => {
            let trimmed = remaining.fragment().trim();
            if !trimmed.is_empty() {
                Err(ParseError::TrailingContent {
                    src: input.to_string(),
                    span: miette::SourceSpan::new(
                        remaining.location_offset().into(),
                        trimmed.len().min(50),
                    ),
                })
            } else {
                Ok(file)
            }
        }
        Err(NomErr::Error(e)) | Err(NomErr::Failure(e)) => Err(e.to_parse_error(input)),
        Err(NomErr::Incomplete(_)) => Err(ParseError::UnexpectedEof),
    }
}

/// Parse ritobin text directly to BinTree.
pub fn parse_to_bin_tree(input: &str) -> Result<BinTree, ParseError> {
    parse(input).map(|f| f.to_bin_tree())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_types() {
        let input = r#"
#PROP_text
type: string = "PROP"
version: u32 = 3
"#;
        let file = parse(input).unwrap();
        assert_eq!(file.file_type(), Some("PROP"));
        assert_eq!(file.version(), Some(3));
    }

    #[test]
    fn test_parse_list() {
        let input = r#"
linked: list[string] = {
    "path/to/file1.bin"
    "path/to/file2.bin"
}
"#;
        let file = parse(input).unwrap();
        let linked = file.linked();
        assert_eq!(linked.len(), 2);
        assert_eq!(linked[0], "path/to/file1.bin");
    }

    #[test]
    fn test_parse_vec3() {
        let input = r#"
pos: vec3 = { 1.0, 2.5, -3.0 }
"#;
        let file = parse(input).unwrap();
        let prop = file.entries.get("pos").unwrap();
        if let PropertyValueEnum::Vector3(Vector3Value(v)) = &prop.value {
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.5);
            assert_eq!(v.z, -3.0);
        } else {
            panic!("Expected Vector3");
        }
    }

    #[test]
    fn test_parse_embed() {
        let input = r#"
data: embed = TestClass {
    name: string = "test"
    value: u32 = 42
}
"#;
        let file = parse(input).unwrap();
        let prop = file.entries.get("data").unwrap();
        if let PropertyValueEnum::Embedded(EmbeddedValue(s)) = &prop.value {
            assert_eq!(s.class_hash, hash_lower("TestClass"));
            assert_eq!(s.properties.len(), 2);
        } else {
            panic!("Expected Embedded");
        }
    }

    #[test]
    fn test_error_reporting() {
        let input = r#"
test: unknowntype = 42
"#;
        let err = parse(input).unwrap_err();
        // The error should be an UnknownType with span info
        match err {
            ParseError::UnknownType {
                type_name, span, ..
            } => {
                assert_eq!(type_name, "unknowntype");
                // Span should point to "unknowntype"
                assert!(span.offset() > 0);
            }
            _ => panic!("Expected UnknownType error, got: {:?}", err),
        }
    }
}
