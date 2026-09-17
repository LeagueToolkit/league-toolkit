//! The two text forms of a [`ValuePath`] beside the hash form: the client path and the named form.

use std::{
    borrow::Cow,
    fmt::{self, Write as _},
};

use ltk_hash::{BinHash, Hash as _};

use super::{MapKey, ValuePath, ValueSegment};
use crate::{
    path::{parse, FieldNames, KeyLiteral, PropertyPath, PropertyPathError, PropertyPathErrorKind},
    property::Kind,
    walk::Leaf,
};

/// A best-effort readable rendering of a [`ValuePath`].
///
/// Created by [`ValuePath::to_named`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedPath {
    /// The text. Every hash the table spelled is a name. Every other is hex, exactly as the hash
    /// form writes it.
    pub text: String,
    /// Hashes the table spelled: field hashes and `Hash`-kind keys.
    pub named: usize,
    /// Hashes it did not spell.
    pub unnamed: usize,
}

impl NamedPath {
    /// Whether every hash was spelled.
    ///
    /// A complete named path names the same position as the client path
    /// [`ValuePath::to_property_path`] produces, where there is one.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unnamed == 0
    }
}

impl fmt::Display for NamedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A position [`ValuePath::to_property_path`] cannot spell.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} (segment {segment})")]
pub struct Nameless {
    /// The index into [`ValuePath::segments`] of the first segment that cannot be spelled. 0 for a path
    /// with no segments.
    pub segment: usize,
    /// Why it cannot be spelled.
    pub kind: NamelessKind,
}

/// The kinds of [`Nameless`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NamelessKind {
    /// The table has no plaintext for `field`, or none that is a property name hashing back to
    /// it. `class` is the class the table was asked with.
    #[error("no name for field {field:08x}")]
    Field {
        /// The field hash.
        field: BinHash,
        /// The class of the node the field was read on, where the path knows it.
        class: Option<BinHash>,
    },
    /// A map key of a kind the path grammar has no `{...}` literal for, or a float key with no
    /// JSON number: `NaN` or an infinity.
    #[error("no literal for a {0:?} key")]
    Key(Kind),
    /// The segments spell no property path: the path is empty, it begins with a subscript, a
    /// subscript follows a subscript, an index is past `u32::MAX`, or the text is longer than
    /// [`PropertyPath::MAX_LEN`].
    #[error("not a property path: {0}")]
    Path(PropertyPathError),
}

impl ValuePath {
    /// The client path naming the same position, if every field has a name and every key a
    /// literal.
    ///
    /// A field is spelled with the name `names` gives for it and its class, when that name is a
    /// property name that hashes back to the field. A key is written as the JSON literal the
    /// client parses: an integer or a float as a number, a bool as `true` or `false`, a string as
    /// a JSON string, and a `Hash` or `File` key as its raw value in decimal.
    ///
    /// # Errors
    ///
    /// [`Nameless`] at the first segment that cannot be spelled.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use ltk_hash::{BinHash, Hash as _};
    /// use ltk_meta::path::{MapKey, ValueSegment, NamelessKind, ValuePath};
    ///
    /// let lookup = BinHash::hash_str("Lookup");
    /// let names = HashMap::from([(lookup, "Lookup".to_owned())]);
    ///
    /// let path: ValuePath = [ValueSegment::Field(lookup), ValueSegment::Key(MapKey::String("weapon".into()))]
    ///     .into_iter()
    ///     .collect();
    /// assert_eq!(path.to_property_path(&names)?.as_str(), r#"Lookup{"weapon"}"#);
    ///
    /// let unknown: ValuePath = [ValueSegment::Field(BinHash(0x1234))].into_iter().collect();
    /// let error = unknown.to_property_path(&names).unwrap_err();
    /// assert!(matches!(error.kind, NamelessKind::Field { .. }));
    /// # Ok::<(), ltk_meta::path::Nameless>(())
    /// ```
    pub fn to_property_path(&self, names: &dyn FieldNames) -> Result<PropertyPath, Nameless> {
        let grammar = |segment, error| Nameless {
            segment,
            kind: NamelessKind::Path(error),
        };
        let no_property = |segment| {
            grammar(
                segment,
                PropertyPathError::new(0, PropertyPathErrorKind::EmptySegment),
            )
        };

        let mut path: Option<PropertyPath> = None;
        let mut fields = self.fields();
        for (segment, item) in self.segments.iter().enumerate() {
            match item {
                ValueSegment::Field(_) => {
                    let (field, class) = fields.next().expect("one class per field segment");
                    let name = spell_field(names, field, class).ok_or(Nameless {
                        segment,
                        kind: NamelessKind::Field { field, class },
                    })?;
                    match &mut path {
                        Some(path) => path.push_field(&name),
                        None => PropertyPath::new(name.into_owned()).map(|new| path = Some(new)),
                    }
                    .map_err(|error| grammar(segment, error))?;
                }
                ValueSegment::Index(index) => {
                    let path = path.as_mut().ok_or_else(|| no_property(segment))?;
                    let index = u32::try_from(*index).map_err(|_| {
                        grammar(
                            segment,
                            PropertyPathError::new(path.len(), PropertyPathErrorKind::InvalidIndex),
                        )
                    })?;
                    path.push_index(index)
                        .map_err(|error| grammar(segment, error))?;
                }
                ValueSegment::Key(key) => {
                    let path = path.as_mut().ok_or_else(|| no_property(segment))?;
                    let number;
                    let literal = match key {
                        MapKey::Bool(v) => KeyLiteral::Bool(*v),
                        MapKey::String(v) => KeyLiteral::String(Cow::Borrowed(v)),
                        MapKey::F32(v) if v.get().is_finite() => {
                            number = v.get().to_string();
                            KeyLiteral::Number(&number)
                        }
                        other => {
                            number = decimal(other).ok_or(Nameless {
                                segment,
                                kind: NamelessKind::Key(other.kind()),
                            })?;
                            KeyLiteral::Number(&number)
                        }
                    };
                    path.push_key(&literal)
                        .map_err(|error| grammar(segment, error))?;
                }
            }
        }
        path.ok_or_else(|| no_property(0))
    }

    /// The path for reading: every hash `names` can spell, spelled; the rest left as hex.
    ///
    /// The grammar is the hash form's. A field is its name where the table has one that hashes
    /// back to it, and a `Hash`-kind key is its plaintext as a JSON string where the table has
    /// one that hashes back to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use ltk_hash::{BinHash, Hash as _};
    /// use ltk_meta::path::{ValueSegment, ValuePath};
    ///
    /// let size = BinHash::hash_str("Size");
    /// let names = HashMap::from([(size, "Size".to_owned())]);
    /// let path: ValuePath = [ValueSegment::Field(BinHash(0x1e6b_a0c4)), ValueSegment::Field(size)]
    ///     .into_iter()
    ///     .collect();
    ///
    /// let named = path.to_named(&names);
    /// assert_eq!(named.text, "1e6ba0c4.Size");
    /// assert_eq!((named.named, named.unnamed), (1, 1));
    /// ```
    #[must_use]
    pub fn to_named(&self, names: &dyn FieldNames) -> NamedPath {
        let mut named = NamedPath {
            text: String::new(),
            named: 0,
            unnamed: 0,
        };
        let mut fields = self.fields();
        for (i, segment) in self.segments.iter().enumerate() {
            let spelled = match segment {
                ValueSegment::Field(_) => {
                    let (field, class) = fields.next().expect("one class per field segment");
                    if i > 0 {
                        named.text.push('.');
                    }
                    let name = spell_field(names, field, class);
                    match &name {
                        Some(name) => named.text.push_str(name),
                        None => push_fmt(&mut named.text, format_args!("{field:08x}")),
                    }
                    Some(name.is_some())
                }
                ValueSegment::Index(_) => {
                    push_fmt(&mut named.text, format_args!("{segment}"));
                    None
                }
                ValueSegment::Key(MapKey::Hash(hash)) => {
                    let text = spell_hash(names, *hash);
                    let key = match &text {
                        Some(text) => Leaf::String(text),
                        None => Leaf::Hash(*hash),
                    };
                    push_fmt(&mut named.text, format_args!("{{{}}}", KeyText(key)));
                    Some(text.is_some())
                }
                ValueSegment::Key(_) => {
                    push_fmt(&mut named.text, format_args!("{segment}"));
                    None
                }
            };
            match spelled {
                Some(true) => named.named += 1,
                Some(false) => named.unnamed += 1,
                None => {}
            }
        }
        named
    }
}

/// The name `names` has for `field`, if it is a property name that hashes back to `field`.
fn spell_field(
    names: &dyn FieldNames,
    field: BinHash,
    class: Option<BinHash>,
) -> Option<Cow<'_, str>> {
    names.field(field, class).filter(|name| {
        !name.is_empty()
            && name.chars().all(parse::is_name_char)
            && BinHash::hash_str(name.as_ref()) == field
    })
}

/// The plaintext `names` has for a `Hash`-kind key, if it hashes back to `hash`.
fn spell_hash(names: &dyn FieldNames, hash: BinHash) -> Option<Cow<'_, str>> {
    names
        .hash(hash)
        .filter(|text| BinHash::hash_str(text.as_ref()) == hash)
}

/// The raw value of an integer, `Hash` or `File` key in decimal. `None` for a kind with no
/// number literal.
fn decimal(key: &MapKey) -> Option<String> {
    Some(match key {
        MapKey::I8(v) => v.to_string(),
        MapKey::U8(v) => v.to_string(),
        MapKey::I16(v) => v.to_string(),
        MapKey::U16(v) => v.to_string(),
        MapKey::I32(v) => v.to_string(),
        MapKey::U32(v) => v.to_string(),
        MapKey::I64(v) => v.to_string(),
        MapKey::U64(v) => v.to_string(),
        MapKey::Hash(v) => v.0.to_string(),
        MapKey::File(v) => v.0.to_string(),
        MapKey::None
        | MapKey::Bool(_)
        | MapKey::F32(_)
        | MapKey::Vector2(_)
        | MapKey::Vector3(_)
        | MapKey::Vector4(_)
        | MapKey::Matrix44(_)
        | MapKey::Color(_)
        | MapKey::String(_) => return None,
    })
}

/// A leaf written as the text inside a `{key}` segment of the hash form.
struct KeyText<'a>(Leaf<'a>);

impl fmt::Display for KeyText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.write_key(f)
    }
}

fn push_fmt(text: &mut String, args: fmt::Arguments<'_>) {
    text.write_fmt(args)
        .expect("writing to a String never fails");
}
