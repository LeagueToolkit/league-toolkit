//! The reads of a recipe, and the names they assign to the keys of a file.

mod claim;
mod hash;
mod part;
mod section;

use std::borrow::Cow;

use ltk_ini::{Inibin, Value};

pub(crate) use part::{AsPart, Part, Parts};
pub(crate) use section::Sec;

use crate::{History, Names, Options, Ty};
use claim::Claims;
use hash::NameHash;
use section::Sections;

/// The result of a read that a loop condition tests.
pub(crate) trait Found {
    fn found(&self) -> bool;
}

impl<T> Found for Option<T> {
    fn found(&self) -> bool {
        self.is_some()
    }
}

impl Found for bool {
    fn found(&self) -> bool {
        *self
    }
}

/// A file as a recipe reads it. Each read of a key that the file has assigns a name to the key.
///
/// A read hashes the parts of the name from the state of the section. A read allocates the
/// text of a name only for a key that the file has.
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    inibin: &'a Inibin,
    history: Option<&'a History>,
    sections: Sections,
    claims: Claims,
    /// `true` on a stale path.
    stale: bool,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(inibin: &'a Inibin, history: Option<&'a History>) -> Self {
        Self {
            inibin,
            history,
            sections: Sections::default(),
            claims: Claims::default(),
            stale: false,
        }
    }

    /// Returns the section `name`. Ignores ASCII case. Removes the whitespace around a name that
    /// is one string, such as a value of the file.
    pub(crate) fn section(&mut self, name: Parts<'_>) -> Sec {
        self.sections.get(name)
    }

    // Lookups.

    /// Names the copies of `section*name` with a comment prefix that the file has. Returns `true`
    /// if the file has such a copy.
    fn claim_commented(
        &mut self,
        section: Sec,
        name: Parts<'_>,
        hash: NameHash,
        ty: Option<Ty>,
    ) -> bool {
        let mut commented = false;
        for (key, comment) in self.sections.commented_keys(section, hash) {
            if self.inibin.contains_key(key) {
                self.claims
                    .add(key, section, comment, name, ty, !self.stale);
                commented = true;
            }
        }
        commented
    }

    /// Names `section*name` and its copies with a comment prefix that the file has. Returns the
    /// value of `section*name`, and whether the file has a copy with a comment prefix.
    fn lookup(
        &mut self,
        section: Sec,
        name: Parts<'_>,
        ty: Option<Ty>,
    ) -> (Option<&'a Value>, bool) {
        let hash = NameHash::of(name);
        let commented = self.claim_commented(section, name, hash, ty);
        let key = self.sections.key(section, hash);
        let value = self.inibin.get(key);
        if value.is_some() {
            self.claims.add(key, section, "", name, ty, !self.stale);
        }
        (value, commented)
    }

    // Getters. Each names the key it reads.

    /// Reads `section*name` with the type `ty`. Returns the value, or `None` if the file has no
    /// such key.
    pub(crate) fn read(
        &mut self,
        section: Sec,
        name: Parts<'_>,
        ty: Option<Ty>,
    ) -> Option<&'a Value> {
        self.lookup(section, name, ty).0
    }

    /// Reads a key with the type `ty`. Returns `true` if the file has the key.
    pub(crate) fn typed(&mut self, section: Sec, name: Parts<'_>, ty: Option<Ty>) -> bool {
        self.read(section, name, ty).is_some()
    }

    /// Reads a key without a declared type. Returns `true` if the file has the key.
    pub(crate) fn key(&mut self, section: Sec, name: Parts<'_>) -> bool {
        self.read(section, name, None).is_some()
    }

    /// Reads a string key. Returns the string, or the text of a value of another storage type.
    /// Returns `None` if the file has no such key.
    ///
    /// The ini compiler stores a string of digits as a number: an emitter `123451` is an
    /// integer value of `GroupPart1`.
    pub(crate) fn string(&mut self, section: Sec, name: Parts<'_>) -> Option<Cow<'a, str>> {
        self.read(section, name, Some(Ty::String))
            .map(|value| match value {
                Value::String(text) => Cow::Borrowed(text.as_str()),
                other => Cow::Owned(other.to_string()),
            })
    }

    /// Reads an integer key. Returns `None` if the file has no such key, or if the value is not a
    /// number.
    pub(crate) fn int(&mut self, section: Sec, name: Parts<'_>) -> Option<i64> {
        self.read(section, name, Some(Ty::Int))
            .and_then(Value::to_i64)
    }

    /// Reads a float key. Returns `None` if the file has no such key, or if the value is not a
    /// number.
    pub(crate) fn float(&mut self, section: Sec, name: Parts<'_>) -> Option<f32> {
        self.read(section, name, Some(Ty::Float))
            .and_then(Value::to_f32)
    }

    pub(crate) fn vec2(&mut self, section: Sec, name: Parts<'_>) -> bool {
        self.typed(section, name, Some(Ty::Vec2))
    }

    pub(crate) fn vec3(&mut self, section: Sec, name: Parts<'_>) -> bool {
        self.typed(section, name, Some(Ty::Vec3))
    }

    pub(crate) fn vec4(&mut self, section: Sec, name: Parts<'_>) -> bool {
        self.typed(section, name, Some(Ty::Vec4))
    }

    /// Returns `true` if the file has `section*name` or a copy of it with a comment prefix. Names
    /// only the copies with a comment prefix, as stale names.
    pub(crate) fn has(&mut self, section: Sec, name: Parts<'_>) -> bool {
        let hash = NameHash::of(name);
        let commented = self.stale(|r| r.claim_commented(section, name, hash, None));
        commented || self.inibin.contains_key(self.sections.key(section, hash))
    }

    /// Reads `name<start>`, `name<start + 1>`, and so on, up to the first index that the file has
    /// no key for. Returns the number of keys, or `None` if the file has no key `name<start>`.
    ///
    /// The client stops at a key that has only a copy with a comment prefix. The reads after
    /// that key are stale.
    pub(crate) fn seq(
        &mut self,
        section: Sec,
        name: Parts<'_>,
        start: i64,
        ty: Option<Ty>,
    ) -> Option<i64> {
        let was = self.stale;
        let mut count = 0;
        loop {
            let parts = [Part::Name(name), Part::Int(start + count)];
            let (value, commented) = self.lookup(section, &parts, ty);
            if value.is_none() {
                if !commented {
                    break;
                }
                self.stale = true;
            }
            count += 1;
        }
        self.stale = was;
        (count > 0).then_some(count)
    }

    // Paths.

    /// Runs `read` as a stale path: the names it assigns are stale.
    pub(crate) fn stale<T>(&mut self, read: impl FnOnce(&mut Self) -> T) -> T {
        self.stale_unless(false, read)
    }

    /// Runs `read`. The names it assigns are stale unless `client` is `true`. A stale path inside
    /// a stale path stays stale.
    pub(crate) fn stale_unless<T>(&mut self, client: bool, read: impl FnOnce(&mut Self) -> T) -> T {
        let was = self.stale;
        self.stale = was || !client;
        let result = read(self);
        self.stale = was;
        result
    }

    // Results.

    /// Returns the string values that other versions of the file hold for `section*name`.
    pub(crate) fn history(&self, section: Sec, name: Parts<'_>) -> &'a [String] {
        let key = self.sections.key(section, NameHash::of(name));
        self.history
            .and_then(|history| history.get(&key))
            .map_or(&[], Vec::as_slice)
    }

    /// Returns the best names of each named key, after `options` removed names.
    pub(crate) fn finish(self, options: Options) -> Names {
        self.claims.finish(&self.sections.names(), options)
    }
}
