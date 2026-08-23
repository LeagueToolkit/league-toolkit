//! The property path language: `Position.UIRect.Size`, `Elements[3]`, `Lookup{"weapon"}`.
//!
//! A [`PropertyPath`] addresses one property inside one bin object. It is the language
//! Riot's own `PropertyPathIterator` implements, and the one `PTCH` patch records use to
//! name what they override. It is *not* an RFC 6901 JSON pointer; JSON only shows up in
//! the `{key}` subscript, whose text is a JSON scalar.
//!
//! ```
//! use ltk_meta::path::{PropertyPath, Subscript};
//!
//! let path = PropertyPath::new("Position.Elements[3]")?;
//! let segments: Vec<_> = path.segments().collect();
//!
//! assert_eq!(segments[0].name, "Position");
//! assert_eq!(segments[1].subscript, Some(Subscript::Index(3)));
//! # Ok::<(), ltk_meta::path::PropertyPathError>(())
//! ```

mod parse;

#[cfg(test)]
mod tests;

use std::{
    borrow::{Borrow, Cow},
    fmt::{self, Write as _},
    str::FromStr,
};

use ltk_hash::{BinHash, Hash as _};

/// A validated property path.
///
/// Every `PropertyPath` is well formed: it parses into at least one segment, its brackets
/// balance, its indices are complete non-negative integers and its keys are JSON scalars.
/// The text is kept exactly as it was given, including the casing of names, the radix of
/// indices and any whitespace inside a key, so a file round-trips byte for byte.
///
/// [`PartialEq`], [`Hash`](std::hash::Hash) and [`Ord`] are textual. Two paths that select
/// the same property can therefore compare unequal (`Size` and `size` hash the same but are
/// different text); compare [`Segment::name_hash`] values for a case-insensitive answer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyPath(String);

impl PropertyPath {
    /// The longest path that can be written to a file: `pathLen` on the wire is a `u16`.
    pub const MAX_LEN: usize = u16::MAX as usize;

    /// Parses and validates a property path.
    ///
    /// # Errors
    ///
    /// Returns a [`PropertyPathError`] pointing at the byte where the path stopped making
    /// sense.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::path::PropertyPath;
    ///
    /// assert!(PropertyPath::new("Position.Anchors.Anchor").is_ok());
    /// assert!(PropertyPath::new("Position.").is_err());
    /// ```
    pub fn new(path: impl Into<String>) -> Result<Self, PropertyPathError> {
        let path = path.into();
        parse::validate(&path)?;
        Ok(Self(path))
    }

    /// Returns the path as it is written.
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the length of the path in bytes, as it is written to a file.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`: an empty path does not parse, so it cannot be constructed.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns an iterator over the segments of the path.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::path::PropertyPath;
    ///
    /// let path = PropertyPath::new("A.B[1]")?;
    /// assert_eq!(path.segments().count(), 2);
    /// # Ok::<(), ltk_meta::path::PropertyPathError>(())
    /// ```
    #[must_use]
    #[inline]
    pub fn segments(&self) -> Segments<'_> {
        Segments(parse::Parser::new(&self.0))
    }

    /// Appends `.name` to the path.
    ///
    /// # Errors
    ///
    /// Returns a [`PropertyPathError`] if the result is not a valid path, in which case the
    /// path is left unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::path::PropertyPath;
    ///
    /// let mut path = PropertyPath::new("Position")?;
    /// path.push_field("Size")?;
    /// assert_eq!(path.as_str(), "Position.Size");
    /// # Ok::<(), ltk_meta::path::PropertyPathError>(())
    /// ```
    pub fn push_field(&mut self, name: &str) -> Result<(), PropertyPathError> {
        // The name is one segment, so a `.` in it would silently add another.
        let at = self.0.len() + 1;
        if name.is_empty() {
            return Err(PropertyPathError::new(
                at,
                PropertyPathErrorKind::EmptySegment,
            ));
        }
        if let Some((offset, c)) = name.char_indices().find(|&(_, c)| !parse::is_name_char(c)) {
            return Err(PropertyPathError::new(
                at + offset,
                PropertyPathErrorKind::UnexpectedCharacter(c),
            ));
        }

        self.push(format_args!(".{name}"))
    }

    /// Appends a `[index]` subscript to the last segment, written in decimal.
    ///
    /// # Errors
    ///
    /// Returns a [`PropertyPathError`] if the last segment already has a subscript, in which
    /// case the path is left unchanged.
    pub fn push_index(&mut self, index: u32) -> Result<(), PropertyPathError> {
        self.push(format_args!("[{index}]"))
    }

    /// Appends a `{key}` subscript to the last segment, written as JSON.
    ///
    /// # Errors
    ///
    /// Returns a [`PropertyPathError`] if the last segment already has a subscript, in which
    /// case the path is left unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::path::{KeyLiteral, PropertyPath};
    ///
    /// let mut path = PropertyPath::new("PerAttachmentMaterial")?;
    /// path.push_key(&KeyLiteral::from("weapon"))?;
    /// assert_eq!(path.as_str(), r#"PerAttachmentMaterial{"weapon"}"#);
    /// # Ok::<(), ltk_meta::path::PropertyPathError>(())
    /// ```
    pub fn push_key(&mut self, key: &KeyLiteral<'_>) -> Result<(), PropertyPathError> {
        self.push(format_args!("{{{key}}}"))
    }

    fn push(&mut self, piece: fmt::Arguments<'_>) -> Result<(), PropertyPathError> {
        let candidate = format!("{}{}", self.0, piece);
        parse::validate(&candidate)?;
        self.0 = candidate;
        Ok(())
    }
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for PropertyPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for PropertyPath {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for PropertyPath {
    type Err = PropertyPathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for PropertyPath {
    type Error = PropertyPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for PropertyPath {
    type Error = PropertyPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PropertyPath> for String {
    fn from(value: PropertyPath) -> Self {
        value.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for PropertyPath {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PropertyPath {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(path).map_err(serde::de::Error::custom)
    }
}

/// An iterator over the segments of a [`PropertyPath`].
///
/// Created by [`PropertyPath::segments`].
#[derive(Clone, Debug)]
pub struct Segments<'a>(parse::Parser<'a>);

impl<'a> Iterator for Segments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // The path was validated when the `PropertyPath` was built, so re-parsing cannot fail.
        self.0
            .next_segment()
            .transpose()
            .expect("a validated PropertyPath always re-parses")
    }
}

impl std::iter::FusedIterator for Segments<'_> {}

/// One `name[subscript]` piece of a [`PropertyPath`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment<'a> {
    /// The property name, exactly as written.
    pub name: &'a str,
    /// The subscript applied to the property, if any.
    pub subscript: Option<Subscript<'a>>,
}

impl Segment<'_> {
    /// The hash the name resolves to: FNV-1a of the lowercased name.
    #[must_use]
    #[inline]
    pub fn name_hash(&self) -> BinHash {
        BinHash::hash_str(self.name)
    }
}

impl fmt::Display for Segment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)?;
        match &self.subscript {
            Some(subscript) => write!(f, "{subscript}"),
            None => Ok(()),
        }
    }
}

/// The `[3]` or `{"weapon"}` part of a [`Segment`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subscript<'a> {
    /// `[i]`: an element of a container, or the value inside an optional.
    Index(u32),
    /// `{k}`: an entry of a map.
    Key(KeyLiteral<'a>),
}

impl fmt::Display for Subscript<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index(index) => write!(f, "[{index}]"),
            Self::Key(key) => write!(f, "{{{key}}}"),
        }
    }
}

/// The JSON scalar inside a `{...}` subscript.
///
/// The client parses the text with rapidjson and converts the result to the map's key kind,
/// so a string can select a hashed key and a number can select an integer one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyLiteral<'a> {
    /// `true` or `false`.
    Bool(bool),
    /// A JSON number, kept as the text that was written.
    Number(&'a str),
    /// A JSON string, unescaped.
    String(Cow<'a, str>),
}

impl<'a> From<&'a str> for KeyLiteral<'a> {
    fn from(value: &'a str) -> Self {
        Self::String(Cow::Borrowed(value))
    }
}

impl From<bool> for KeyLiteral<'_> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl fmt::Display for KeyLiteral<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(true) => f.write_str("true"),
            Self::Bool(false) => f.write_str("false"),
            Self::Number(text) => f.write_str(text),
            Self::String(text) => {
                f.write_char('"')?;
                for c in text.chars() {
                    match c {
                        '"' => f.write_str("\\\"")?,
                        '\\' => f.write_str("\\\\")?,
                        '\u{8}' => f.write_str("\\b")?,
                        '\u{c}' => f.write_str("\\f")?,
                        '\n' => f.write_str("\\n")?,
                        '\r' => f.write_str("\\r")?,
                        '\t' => f.write_str("\\t")?,
                        c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
                        c => f.write_char(c)?,
                    }
                }
                f.write_char('"')
            }
        }
    }
}

/// Why a string is not a valid [`PropertyPath`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at byte {offset}")]
pub struct PropertyPathError {
    offset: usize,
    kind: PropertyPathErrorKind,
}

impl PropertyPathError {
    pub(crate) fn new(offset: usize, kind: PropertyPathErrorKind) -> Self {
        Self { offset, kind }
    }

    /// The byte offset in the path where parsing stopped.
    #[must_use]
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// What went wrong.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> PropertyPathErrorKind {
        self.kind
    }
}

/// The kinds of [`PropertyPathError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PropertyPathErrorKind {
    /// A segment has no name, as in `A..B`, `.A`, `A.` or the empty path.
    #[error("expected a property name")]
    EmptySegment,
    /// A character that cannot appear where it does.
    #[error("unexpected character {0:?}")]
    UnexpectedCharacter(char),
    /// A `[` or `{` with no matching close.
    #[error("unbalanced bracket")]
    UnbalancedBracket,
    /// A second subscript on one segment, as in `A[1][2]`.
    #[error("a segment can only have one subscript")]
    DoubleSubscript,
    /// A `[...]` that is not a complete non-negative integer.
    #[error("expected a non-negative integer index")]
    InvalidIndex,
    /// A `{...}` that is not a JSON scalar.
    #[error("expected a JSON number, string or boolean key")]
    InvalidKey,
    /// The path is longer than [`PropertyPath::MAX_LEN`].
    #[error("path is {0} bytes, the limit is 65535")]
    TooLong(usize),
}
