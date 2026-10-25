//! The sections that a recipe reads.

use std::collections::HashMap;
use std::ops::Range;

use ltk_hash::IniHash;

use super::hash::{extend, NameHash};
use super::part::{write, Part, Parts};
use crate::COMMENT_PREFIXES;

/// A section that a recipe reads. [`Reader::section`](super::Reader::section) returns a `Sec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sec(pub(super) usize);

/// A section: its name, and the SDBM state after `Section*` and after `Section*` and each
/// comment prefix.
#[derive(Debug)]
struct Section {
    /// The range of the name in [`Sections::text`].
    name: Range<usize>,
    states: [u32; 1 + COMMENT_PREFIXES.len()],
}

/// The sections that a recipe reads. Two names that differ in ASCII case are one section.
#[derive(Debug, Default)]
pub(super) struct Sections {
    sections: Vec<Section>,
    /// The section of each SDBM state after `Section*`.
    by_state: HashMap<u32, Sec>,
    /// The text of every section name.
    text: Vec<u8>,
}

impl Sections {
    /// Returns the section `name`. Removes the whitespace around a name that is one string, such
    /// as a value of the file.
    pub(super) fn get(&mut self, name: Parts<'_>) -> Sec {
        let trimmed;
        let name = match name {
            [Part::Str(text)] => {
                trimmed = [Part::Str(text.trim())];
                &trimmed[..]
            }
            name => name,
        };
        let state = extend(NameHash::of(name).after(0), "*");
        if let Some(&section) = self.by_state.get(&state) {
            return section;
        }
        let start = self.text.len();
        write(name, &mut self.text);
        let mut states = [state; 1 + COMMENT_PREFIXES.len()];
        for (slot, comment) in states[1..].iter_mut().zip(COMMENT_PREFIXES) {
            *slot = extend(state, comment);
        }
        let section = Sec(self.sections.len());
        self.sections.push(Section {
            name: start..self.text.len(),
            states,
        });
        self.by_state.insert(state, section);
        section
    }

    /// Returns the key of `section*name`.
    pub(super) fn key(&self, section: Sec, name: NameHash) -> IniHash {
        name.key(self.sections[section.0].states[0])
    }

    /// Returns the key of `section*name` with each comment prefix, and the prefix.
    pub(super) fn commented_keys(
        &self,
        section: Sec,
        name: NameHash,
    ) -> impl Iterator<Item = (IniHash, &'static str)> {
        let states = self.sections[section.0].states;
        states
            .into_iter()
            .skip(1)
            .zip(COMMENT_PREFIXES)
            .map(move |(state, comment)| (name.key(state), comment))
    }

    /// Returns the name of each section, indexed by [`Sec`].
    pub(super) fn names(&self) -> Vec<String> {
        self.sections
            .iter()
            .map(|section| String::from_utf8_lossy(&self.text[section.name.clone()]).into_owned())
            .collect()
    }
}
