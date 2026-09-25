use std::{fmt::Display, num::IntErrorKind};

use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{path::PropertyPathError, PropertyKind};

use crate::{
    ast::node::root::{FileKind, RootKind},
    cst,
    escaping::InvalidEscapeReason,
    parse::{Span, TokenKind},
    ItemShape, RitoType, Spanned,
};

#[derive(Debug, Clone, Copy)]
pub enum RitoTypeOrVirtual {
    Unknown,
    RitoType(RitoType),
    Numeric,
    StructOrEmbedded,
    Token(TokenKind),
    Tree(cst::Kind),
}

impl RitoTypeOrVirtual {
    pub fn numeric() -> Self {
        Self::Numeric
    }
}

impl From<Option<RitoType>> for RitoTypeOrVirtual {
    fn from(value: Option<RitoType>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::Unknown,
        }
    }
}

impl From<RitoType> for RitoTypeOrVirtual {
    fn from(value: RitoType) -> Self {
        RitoTypeOrVirtual::RitoType(value)
    }
}

impl Display for RitoTypeOrVirtual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => f.write_str("unknown type"),
            Self::RitoType(rito_type) => Display::fmt(rito_type, f),
            Self::Numeric => f.write_str("numeric type"),
            Self::StructOrEmbedded => f.write_str("struct/embedded"),
            Self::Token(kind) => Display::fmt(kind, f),
            Self::Tree(kind) => Display::fmt(kind, f),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ListLike {
    Color,
    Vec2,
    Vec3,
    Vec4,
    Mat44,
}

impl ListLike {
    /// The listlike `kind` spells out, if it is one.
    pub fn from_kind(kind: PropertyKind) -> Option<Self> {
        Some(match kind {
            PropertyKind::Color => ListLike::Color,
            PropertyKind::Vector2 => ListLike::Vec2,
            PropertyKind::Vector3 => ListLike::Vec3,
            PropertyKind::Vector4 => ListLike::Vec4,
            PropertyKind::Matrix44 => ListLike::Mat44,
            _ => return None,
        })
    }

    pub fn needed_children(&self) -> u8 {
        match self {
            ListLike::Color => 4,
            ListLike::Vec2 => 2,
            ListLike::Vec3 => 3,
            ListLike::Vec4 => 4,
            ListLike::Mat44 => 16,
        }
    }
}

impl Display for ListLike {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        RitoType::simple(match self {
            ListLike::Color => PropertyKind::Color,
            ListLike::Vec2 => PropertyKind::Vector2,
            ListLike::Vec3 => PropertyKind::Vector3,
            ListLike::Vec4 => PropertyKind::Vector4,
            ListLike::Mat44 => PropertyKind::Matrix44,
        })
        .fmt(f)
    }
}

/// A field of a `patch` embed, one record of a `PTCH` file's `patches` root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchField {
    /// `path: string`, the property the record patches.
    Path,
    /// `value`, the value the record writes.
    Value,
}

impl PatchField {
    /// Both fields, in the order a `patch` embed writes them.
    pub const ALL: [Self; 2] = [Self::Path, Self::Value];

    /// The field's name in a `patch` embed.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Value => "value",
        }
    }

    /// The field whose name hashes to `name`, compared as bin field names are: FNV-1a of the
    /// ASCII-lowercased name.
    pub fn named(name: BinHash) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|field| BinHash::hash_str(field.as_str()) == name)
    }
}

impl Display for PatchField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Something the type checker found wrong with a tree.
///
/// Each variant carries the spans needed to point at the offending source. Use [`Display`] to render the user-facing message.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Diagnostic {
    CustomSpan(&'static str, Span),

    InvalidEscape {
        /// Span of the offending string literal
        span: Span,
        /// index of the invalid `\`, relative to `span.start`
        offset: u32,
        reason: InvalidEscapeReason,
    },

    UnexpectedTree {
        tree: cst::Kind,
        expected: Option<cst::Kind>,
        span: Span,
    },
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),

    MissingToken(TokenKind),
    UnknownType(Span),
    MissingType(Span),

    TypeMismatch {
        span: Span,
        expected: RitoTypeOrVirtual,
        expected_span: Option<Span>,
        got: RitoTypeOrVirtual,
    },

    UnexpectedContainerItem {
        span: Span,
        expected: RitoType,
        expected_span: Option<Span>,
    },

    /// A value needing a class name was written as a bare block.
    ///
    /// `pointer` and `embed` values are `ClassName { .. }`; without the name there is
    /// nothing to resolve the class from, and the value defaults to class hash 0.
    MissingClassName {
        /// span of the `{` the class name should have preceded
        span: Span,
        /// the type that requires the class name
        expected: RitoType,
    },

    /// An item has the wrong shape for its parent.
    ///
    /// A bare value in a class body, an entry in a list, and so on. The item cannot be
    /// merged into the parent, so it is missing from the bin.
    UnexpectedItem {
        /// span of the offending item
        span: Span,
        /// the parent that rejected it
        parent: RitoType,
        /// the shape `parent` needs
        expected: ItemShape,
    },

    /// A property name was written with quotes.
    QuotedPropertyName {
        /// span of the quoted key, quotes included
        span: Span,
        /// the parent class
        parent: RitoType,
    },

    ResolveLiteral,
    ParseNumericError {
        expected: PropertyKind,
        error: Option<std::num::IntErrorKind>,
        span: Span,
    },
    AmbiguousNumeric(Span),

    NotEnoughItems {
        span: Span,
        got: u8,
        expected: ListLike,
    },
    TooManyItems {
        span: Span,
        extra: u8,
        expected: ListLike,
    },

    /// Root entry is not a valid entry (key: type = value)
    RootNonEntry,
    /// Root entry is not recognised
    UnknownRoot {
        /// span of the unrecognised entry's name
        span: Span,
    },
    MissingRootEntry {
        root_kind: RootKind,
    },
    /// An entry is missing its value
    MissingEntryValue {
        key_span: Span,
        expected: Option<Spanned<RitoType>>,
    },
    MissingEntryType {
        key_span: Span,
    },

    InvalidRootEntryType {
        root_kind: RootKind,
        key_span: Span,
        type_span: Span,
        got: RitoTypeOrVirtual,
        expected: RitoType,
    },
    /// A root only a `PTCH` file has, in a file of another kind.
    PatchOnlyRoot {
        /// span of the root's name
        span: Span,
        root_kind: RootKind,
    },
    /// A `PTCH` file whose `linked` root is not empty. The client cannot load a patch that
    /// links other bins.
    PatchLinked {
        /// span of the `linked` root's value
        span: Span,
    },
    /// A `PTCH` file whose `version` root is not 3, the only version of `PTCH` text.
    UnsupportedPatchVersion {
        /// span of the `version` root's value
        span: Span,
        version: u32,
    },
    /// A `patches` record whose embed class is not `patch`.
    UnexpectedPatchClass {
        /// span of the class name
        span: Span,
    },
    /// A `patch` embed without one of its fields. The record is left out of the patch.
    MissingPatchField {
        /// span of the `patch` class name
        span: Span,
        field: PatchField,
    },
    /// A second `path` or `value` field in one `patch` embed. The first one is used.
    DuplicatePatchField {
        /// span of the second field's name
        span: Span,
        field: PatchField,
    },
    /// A field of a `patch` embed other than `path` and `value`.
    UnexpectedPatchField {
        /// span of the whole field
        span: Span,
    },
    /// A record `path` that is not a valid property path. The record is left out of the patch.
    InvalidPropertyPath {
        /// span of the path's string literal
        span: Span,
        error: PropertyPathError,
    },
    /// A file built as a kind its `type` root does not name, such as a `PTCH` file handed to
    /// [`crate::Cst::build_bin`].
    UnexpectedFileKind {
        /// span of the `type` root's value
        span: Span,
        expected: FileKind,
        found: FileKind,
    },

    ShadowedEntry {
        shadowee: Span,
        shadower: Span,
    },
    /// [`Self::ShadowedEntry`], but for a pair of root indices
    ShadowedRoot {
        shadowee: usize,
        shadower: usize,
    },

    InvalidHash(Span),

    SubtypeCountMismatch {
        span: Span,
        got: u8,
        expected: u8,
    },
    /// Subtypes found on a type that has no subtypes
    UnexpectedSubtypes {
        span: Span,
        base_type: Span,
    },

    /// A container/map/optional's declared item type is itself container-shaped
    /// (list/list2/map/option)
    InvalidNesting {
        /// span of the offending subtype token
        span: Span,
        /// the container-shaped type that cannot be nested
        kind: RitoType,
    },
    /// A map's declared key type cannot key a map, such as `map[link,u32]`
    InvalidMapKey {
        /// span of the offending key subtype token
        span: Span,
        /// the type that cannot key a map
        kind: RitoType,
    },
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Diagnostic::*;
        match self {
            CustomSpan(msg, _) => f.write_str(msg),

            Self::InvalidEscape { .. } => write!(f, "Invalid escape character"),
            UnexpectedTree {
                tree,
                expected: Some(expected),
                ..
            } => write!(f, "Unexpected {tree}, expected {expected}"),
            UnexpectedTree {
                tree,
                expected: None,
                ..
            } => write!(f, "Unexpected {tree}"),
            MissingTree(kind) => write!(f, "Missing {kind}"),
            EmptyTree(kind) => write!(f, "Empty {kind}"),
            MissingToken(kind) => write!(f, "Missing {kind}"),

            UnknownType(_) => f.write_str("Unknown type"),
            MissingType(_) => {
                f.write_str("Missing type - entries are written 'name: type = value'")
            }

            TypeMismatch { expected, got, .. } => {
                write!(f, "Type mismatch - expected {expected}, got {got}")
            }
            UnexpectedContainerItem { expected, .. } => write!(
                f,
                "{expected} does not accept container items / blocks - remove the curly \
                 braces around the value"
            ),
            MissingClassName { expected, .. } => write!(
                f,
                "Missing class name - {expected} values are written 'ClassName {{ .. }}'"
            ),
            UnexpectedItem {
                parent, expected, ..
            } => write!(f, "{parent} takes {expected}"),
            QuotedPropertyName { parent, .. } => write!(
                f,
                "Quoted property name - {parent} bodies take 'name: type = value', with the \
                 name unquoted or a '0x..' hash"
            ),

            ResolveLiteral => f.write_str("Could not resolve literal"),
            ParseNumericError {
                expected, error, ..
            } => {
                let reason = match error {
                    Some(IntErrorKind::Empty) => "cannot parse integer from empty literal",
                    Some(IntErrorKind::InvalidDigit) => "invalid digit found in literal",
                    Some(IntErrorKind::PosOverflow) => "number too large to fit in target type",
                    Some(IntErrorKind::NegOverflow) => "number too small to fit in target type",
                    Some(IntErrorKind::Zero) => "number would be zero",
                    _ => "invalid literal",
                };
                write!(
                    f,
                    "Could not parse {} - {reason}",
                    RitoType::simple(*expected)
                )
            }
            AmbiguousNumeric(_) => {
                f.write_str("Ambiguous numeric literal - it needs a type to be resolved against")
            }

            NotEnoughItems { got, expected, .. } => write!(
                f,
                "{expected} needs {} items - got {got}",
                expected.needed_children()
            ),
            TooManyItems {
                extra, expected, ..
            } => write!(
                f,
                "{expected} needs {} items - got {extra} too many",
                expected.needed_children()
            ),

            RootNonEntry => f.write_str("Top-level bin entries are written 'name: type = value'"),
            UnknownRoot { .. } => f.write_str("Unknown root entry"),
            MissingRootEntry { root_kind } => write!(f, "Missing root entry '{root_kind}'"),
            InvalidRootEntryType {
                root_kind,
                got,
                expected,
                ..
            } => write!(f, "Root entry '{root_kind}' must be {expected}, got {got}"),
            PatchOnlyRoot { root_kind, .. } => {
                write!(f, "Root entry '{root_kind}' only belongs in a PTCH file")
            }
            PatchLinked { .. } => {
                f.write_str("A PTCH file cannot link other bins - 'linked' must be empty")
            }
            UnsupportedPatchVersion { version, .. } => {
                write!(
                    f,
                    "Unsupported PTCH version {version} - the only version is 3"
                )
            }
            UnexpectedPatchClass { .. } => f.write_str(
                "Patch records are written 'patch { path: string = .., value: type = .. }'",
            ),
            MissingPatchField { field, .. } => write!(f, "Patch record is missing '{field}'"),
            DuplicatePatchField { field, .. } => {
                write!(f, "Patch record already has a '{field}'")
            }
            UnexpectedPatchField { .. } => {
                f.write_str("Patch records only have the fields 'path' and 'value'")
            }
            InvalidPropertyPath { error, .. } => write!(f, "Invalid property path - {error}"),
            UnexpectedFileKind {
                expected, found, ..
            } => write!(f, "Expected a {expected} file, found a {found} file"),
            MissingEntryValue {
                key_span: _,
                expected,
            } => {
                f.write_str("Entry is missing value")?;
                if let Some(expected) = expected {
                    write!(f, " (expected {})", expected.value)?;
                }
                Ok(())
            }
            MissingEntryType { key_span: _ } => f.write_str("Entry is missing type expression"),
            ShadowedEntry { .. } | ShadowedRoot { .. } => {
                f.write_str("Entry shadows a previous entry with the same key")
            }

            InvalidHash(_) => f.write_str("Invalid hash"),

            SubtypeCountMismatch { got, expected, .. } => {
                write!(f, "Expected {expected} type parameters, got {got}")
            }
            UnexpectedSubtypes { .. } => f.write_str("This type does not accept type parameters"),

            InvalidNesting { kind, .. } => {
                write!(f, "{kind} cannot be nested inside a container")
            }
            InvalidMapKey { kind, .. } => {
                write!(f, "{kind} is not a valid map key type")
            }
        }
    }
}

impl Diagnostic {
    pub fn span(&self) -> Option<Span> {
        use Diagnostic::*;
        match self {
            MissingTree(_)
            | EmptyTree(_)
            | MissingToken(_)
            | RootNonEntry
            | ShadowedRoot { .. }
            | ResolveLiteral
            | MissingRootEntry { .. } => None,
            InvalidEscape { span, offset, .. } => Some(Span::new(
                span.start + offset,
                (span.start + offset + 1).min(span.end),
            )),
            UnknownType(span)
            | UnknownRoot { span }
            | UnexpectedTree { span, .. }
            | CustomSpan(_, span)
            | SubtypeCountMismatch { span, .. }
            | UnexpectedSubtypes { span, .. }
            | UnexpectedContainerItem { span, .. }
            | MissingClassName { span, .. }
            | UnexpectedItem { span, .. }
            | QuotedPropertyName { span, .. }
            | MissingType(span)
            | TypeMismatch { span, .. }
            | MissingEntryType { key_span: span, .. }
            | MissingEntryValue { key_span: span, .. }
            | ShadowedEntry { shadower: span, .. }
            | InvalidHash(span)
            | AmbiguousNumeric(span)
            | ParseNumericError { span, .. }
            | NotEnoughItems { span, .. }
            | TooManyItems { span, .. }
            | InvalidNesting { span, .. }
            | InvalidMapKey { span, .. }
            | PatchOnlyRoot { span, .. }
            | PatchLinked { span }
            | UnsupportedPatchVersion { span, .. }
            | UnexpectedPatchClass { span }
            | MissingPatchField { span, .. }
            | DuplicatePatchField { span, .. }
            | UnexpectedPatchField { span }
            | InvalidPropertyPath { span, .. }
            | UnexpectedFileKind { span, .. }
            | InvalidRootEntryType { key_span: span, .. } => Some(*span),
        }
    }

    pub fn default_span(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().unwrap_or(span),
            diagnostic: self,
        }
    }

    pub fn unwrap(self) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().unwrap(),
            diagnostic: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiagnosticWithSpan {
    pub diagnostic: Diagnostic,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct MaybeSpanDiag {
    pub diagnostic: Diagnostic,
    pub span: Option<Span>,
}

impl MaybeSpanDiag {
    pub fn fallback(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span.unwrap_or(span),
            diagnostic: self.diagnostic,
        }
    }
}

impl From<Diagnostic> for MaybeSpanDiag {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            span: diagnostic.span(),
            diagnostic,
        }
    }
}
