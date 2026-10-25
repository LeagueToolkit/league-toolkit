/// The type that a recipe declares for a property. The client getter of the property reads the
/// value as this type.
///
/// The format has no boolean type. A flag is an `Int` that is 0 or 1, and a file stores it as a
/// `U1` (ADR-0027).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ty {
    Int,
    Float,
    Vec2,
    Vec3,
    Vec4,
    Vec5,
    String,
}

/// Prefixes of a property name that the ini compiler stores with the name. The ini compiler of
/// old builds does not treat a line that starts with one of the prefixes as a comment. The file
/// stores the line as the property `'Name`, `*Name`, and so on. The client never reads such a
/// property.
pub const COMMENT_PREFIXES: [&str; 5] = ltk_ini::ADDITIONAL_COMMENT_TOKENS;

/// The section name and the property name of a key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub section: String,
    /// The property name, including a comment prefix.
    pub name: String,
    /// The declared type. `None` if the recipe does not declare a type.
    pub ty: Option<Ty>,
    /// `true` if the client never reads the key: a typo, a part after a gap, a part of a removed
    /// emitter. A key with a comment prefix is stale.
    pub stale: bool,
}

impl Name {
    /// Returns the comment prefix of the property name. Returns `None` for a property name
    /// without a comment prefix.
    #[must_use]
    pub fn comment(&self) -> Option<&'static str> {
        COMMENT_PREFIXES
            .into_iter()
            .find(|prefix| self.name.starts_with(prefix))
    }
}

/// The keys that [`unhash`](crate::unhash) names.
///
/// The default names every key that a recipe reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Names the keys that the client never reads.
    pub stale: bool,
    /// Names the keys with a comment prefix.
    pub commented: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            stale: true,
            commented: true,
        }
    }
}

impl Options {
    /// Returns options that name only the keys that the client reads.
    #[must_use]
    pub fn client() -> Self {
        Self {
            stale: false,
            commented: false,
        }
    }
}
