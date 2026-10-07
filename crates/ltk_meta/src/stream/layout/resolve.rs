//! Resolves the width of each [`Kind::Hash`] value of a buffer from the sizes that the buffer
//! declares.
//!
//! A `Hash` value occupies 4 or 8 bytes. The file does not store the width. Each sized region
//! declares its byte size. The counts inside a region define the number of its values. The search
//! finds a width for each `Hash` value such that each region ends at its declared size.
//!
//! The search runs only if a walk that reads each `Hash` value as 4 bytes fails.
//!
//! The rule for a `Hash` value depends on its position:
//!
//! - Container item: all items have the same width. The width is the body size divided by the
//!   item count.
//! - Map key or map value: all keys have the same width. All values have the same width. The
//!   search walks the body with each combination of widths. It selects 4-byte keys and 4-byte
//!   values if the body ends at its declared size with them. Otherwise it selects the one other
//!   combination with which the body ends at its declared size.
//! - Property of an object or a struct, or the item of an optional in that position: the search
//!   tries 4 bytes first and 8 bytes second, in file order. It backtracks if the region does not
//!   end at its declared size.
//!
//! The extent of a sized region does not depend on the widths inside the region. The search
//! resolves each region independently. It does not revisit a resolved region.

use ltk_hash::HashWidth;

use crate::{
    property::Kind,
    stream::layout::{Cursor, HashWidths, Numbering},
    Error,
};

/// The result of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// A valid assignment. The record contains the offsets of the 8-byte values. `end` is the
    /// offset after the value or the object.
    Resolved { end: usize },
    /// A map for which 4-byte keys with 4-byte values are not valid and more than one other
    /// combination of key width and value width is valid.
    Ambiguous,
    /// No valid assignment of widths.
    Unresolved,
}

/// The number of search steps that are allowed per byte of the buffer. One walk without
/// backtracking uses at most one step per 2 bytes.
const BUDGET_PER_BYTE: usize = 16;

/// The number of search steps that are allowed in addition to [`BUDGET_PER_BYTE`] steps per byte.
const BUDGET_BASE: usize = 1024;

/// Runs `attempt` with each `Hash` value read as 4 bytes. If `attempt` fails and `numbering` is
/// [`Numbering::Current`], runs `search` and then runs `attempt` with the widths that `search`
/// resolved.
///
/// # Errors
///
/// Returns the error of the first `attempt` if `numbering` is [`Numbering::Legacy`] or if
/// `search` returns [`Resolution::Unresolved`]. Returns [`Error::AmbiguousHashWidth`] if `search`
/// returns [`Resolution::Ambiguous`]. Returns the error of the second `attempt` if the second
/// `attempt` fails.
pub(crate) fn with_resolved_widths<T>(
    buf: &[u8],
    numbering: Numbering,
    widths: &mut HashWidths,
    search: impl FnOnce(&[u8], &mut HashWidths) -> Resolution,
    attempt: impl Fn(Cursor<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    widths.clear();
    let error = match attempt(Cursor::new(buf, numbering)) {
        Ok(value) => return Ok(value),
        Err(error) => error,
    };

    // A file with the legacy numbering was written before the client stored a `Hash` in 8 bytes.
    if numbering.is_legacy() {
        return Err(error);
    }

    match search(buf, widths) {
        Resolution::Resolved { .. } => attempt(Cursor::with_widths(buf, numbering, widths)),
        Resolution::Ambiguous => Err(Error::AmbiguousHashWidth),
        Resolution::Unresolved => Err(error),
    }
}

/// Resolves the widths of one object. `buf` starts at the `u32` size field of the object.
pub(crate) fn search_object(buf: &[u8], widths: &mut HashWidths) -> Resolution {
    Search::run(buf, widths, |search| {
        let size = search.u32(0, buf.len())? as usize;
        let end = bounded(size.checked_add(4)?, buf.len())?;
        search.object_body(4, end)?;
        Some(end)
    })
}

/// Resolves the widths of one object body. `body` starts after the `u32` size field of the
/// object and ends at the end of the object.
pub(crate) fn search_object_body(body: &[u8], widths: &mut HashWidths) -> Resolution {
    Search::run(body, widths, |search| {
        search.object_body(0, body.len())?;
        Some(body.len())
    })
}

/// Resolves the widths of one value of `kind` at the start of `buf`.
///
/// Leaves a `Hash` value at 4 bytes if no declared size contains it. This applies to a `Hash`
/// value at the start of `buf` and to the `Hash` item of an optional at the start of `buf`.
pub(crate) fn search_value(buf: &[u8], kind: Kind, widths: &mut HashWidths) -> Resolution {
    Search::run(buf, widths, |search| search.value(kind, 0, buf.len()))
}

/// A `Hash` property that the search read as 4 bytes and has not read as 8 bytes.
struct Choice {
    /// The index of the property in its region.
    index: u16,
    /// The offset of the `Hash` value.
    at: usize,
    /// The length of the record before the search read the property.
    mark: usize,
}

struct Search<'a> {
    buf: &'a [u8],
    wide: &'a mut Vec<usize>,
    /// The choices of each region that the search is inside. The choices of the innermost region
    /// are last.
    choices: Vec<Choice>,
    /// The number of steps that remain.
    budget: usize,
    /// `true` if the search found a map with more than one valid combination of widths.
    ambiguous: bool,
}

impl<'a> Search<'a> {
    fn run(
        buf: &'a [u8],
        widths: &'a mut HashWidths,
        body: impl FnOnce(&mut Self) -> Option<usize>,
    ) -> Resolution {
        widths.wide.clear();
        let mut search = Search {
            buf,
            wide: &mut widths.wide,
            choices: Vec::new(),
            budget: buf
                .len()
                .saturating_mul(BUDGET_PER_BYTE)
                .saturating_add(BUDGET_BASE),
            ambiguous: false,
        };

        match body(&mut search) {
            Some(end) => Resolution::Resolved { end },
            None => {
                search.wide.clear();
                match search.ambiguous {
                    true => Resolution::Ambiguous,
                    false => Resolution::Unresolved,
                }
            }
        }
    }

    /// Uses one step. Returns `None` if no step remains.
    fn tick(&mut self) -> Option<()> {
        self.charge(1)
    }

    /// Uses `steps` steps. Sets the number of remaining steps to 0 and returns `None` if fewer
    /// than `steps` steps remain.
    fn charge(&mut self, steps: usize) -> Option<()> {
        let rest = self.budget.checked_sub(steps);
        self.budget = rest.unwrap_or(0);
        rest.map(|_| ())
    }

    fn bytes(&self, pos: usize, len: usize, end: usize) -> Option<&'a [u8]> {
        let stop = bounded(pos.checked_add(len)?, end)?;
        self.buf.get(pos..stop)
    }

    fn u8(&self, pos: usize, end: usize) -> Option<u8> {
        Some(self.bytes(pos, 1, end)?[0])
    }

    fn u16(&self, pos: usize, end: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            self.bytes(pos, 2, end)?.try_into().ok()?,
        ))
    }

    fn u32(&self, pos: usize, end: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.bytes(pos, 4, end)?.try_into().ok()?,
        ))
    }

    fn kind(&self, pos: usize, end: usize) -> Option<Kind> {
        Kind::unpack(self.u8(pos, end)?, false).ok()
    }

    /// Reads the kind that a container, an optional or a map declares for its items or values.
    /// Returns `None` if the kind is a container kind.
    fn item_kind(&self, pos: usize, end: usize) -> Option<Kind> {
        self.kind(pos, end).filter(|kind| !kind.is_container())
    }

    /// Reads the `u32` size field of a sized region at `pos`. Returns the offset of the body and
    /// the offset of the end of the body. Returns `None` if the body ends after `end`.
    fn region(&self, pos: usize, end: usize) -> Option<(usize, usize)> {
        let size = self.u32(pos, end)? as usize;
        let body = pos + 4;
        Some((body, bounded(body.checked_add(size)?, end)?))
    }

    fn object_body(&mut self, pos: usize, end: usize) -> Option<()> {
        let count = self.u16(pos.checked_add(4)?, end)?;
        self.properties(pos + 6, end, count)
    }

    /// Walks one value of `kind` at `pos`. Returns the offset after the value.
    ///
    /// Reads a `Hash` value as 4 bytes. A caller that resolves the width of a `Hash` value does
    /// not call this function for that value.
    fn value(&mut self, kind: Kind, pos: usize, end: usize) -> Option<usize> {
        use Kind as K;
        if let Some(width) = kind.fixed_width() {
            return bounded(pos.checked_add(width)?, end);
        }
        match kind {
            K::Hash => bounded(pos.checked_add(4)?, end),
            K::String => {
                let len = self.u16(pos, end)? as usize;
                bounded(pos + 2 + len, end)
            }
            K::Container | K::UnorderedContainer => {
                let item_kind = self.item_kind(pos, end)?;
                let (body, end) = self.region(pos + 1, end)?;
                let count = self.u32(body, end)? as usize;
                self.items(item_kind, count, body + 4, end)?;
                Some(end)
            }
            K::Map => {
                let key_kind = self.kind(pos, end).filter(Kind::is_valid_map_key)?;
                let value_kind = self.item_kind(pos + 1, end)?;
                let (body, end) = self.region(pos + 2, end)?;
                let count = self.u32(body, end)? as usize;
                self.entries(key_kind, value_kind, count, body + 4, end)?;
                Some(end)
            }
            K::Struct | K::Embedded => {
                if self.u32(pos, end)? == 0 {
                    return Some(pos + 4);
                }
                let (body, end) = self.region(pos + 4, end)?;
                let count = self.u16(body, end)?;
                self.properties(body + 2, end, count)?;
                Some(end)
            }
            K::Optional => {
                let item_kind = self.item_kind(pos, end)?;
                match self.u8(pos + 1, end)? != 0 {
                    true => self.value(item_kind, pos + 2, end),
                    false => Some(pos + 2),
                }
            }
            _ => unreachable!("every kind without a fixed width is matched above"),
        }
    }

    /// Walks the `count` items of a container. The items occupy `pos..end`.
    fn items(&mut self, item_kind: Kind, count: usize, pos: usize, end: usize) -> Option<()> {
        let body = end - pos;

        if item_kind == Kind::Hash {
            if count.checked_mul(4) == Some(body) {
                return Some(());
            }
            if count.checked_mul(8) == Some(body) {
                // The record receives `count` offsets. Backtracking can repeat this walk.
                self.charge(count)?;
                self.wide.extend((0..count).map(|index| pos + index * 8));
                return Some(());
            }
            return None;
        }
        if let Some(width) = item_kind.fixed_width() {
            return (count.checked_mul(width) == Some(body)).then_some(());
        }

        let mut pos = pos;
        for _ in 0..count {
            self.tick()?;
            pos = self.value(item_kind, pos, end)?;
        }
        (pos == end).then_some(())
    }

    /// Walks the `count` entries of a map. The entries occupy `pos..end`.
    ///
    /// Selects 4-byte keys and 4-byte values if the entries end at `end` with them. The first
    /// walk reads the same widths. Otherwise selects the one other combination of key width and
    /// value width with which the entries end at `end`.
    ///
    /// Sets [`Search::ambiguous`] and returns `None` if more than one other combination is
    /// valid.
    fn entries(
        &mut self,
        key_kind: Kind,
        value_kind: Kind,
        count: usize,
        pos: usize,
        end: usize,
    ) -> Option<()> {
        const FIXED: &[Option<HashWidth>] = &[None];
        const EITHER: &[Option<HashWidth>] = &[Some(HashWidth::W4), Some(HashWidth::W8)];
        let widths_of = |kind| match kind {
            Kind::Hash => EITHER,
            _ => FIXED,
        };

        if count == 0 {
            return (pos == end).then_some(());
        }

        let mut resolved = None;
        for &key_width in widths_of(key_kind) {
            for &value_width in widths_of(value_kind) {
                let mark = self.wide.len();
                let side = |kind, width| MapSide { kind, width };
                let fits = self
                    .entries_with(
                        side(key_kind, key_width),
                        side(value_kind, value_width),
                        count,
                        pos,
                        end,
                    )
                    .is_some();

                // The loop tries this combination first.
                let narrow = key_width != Some(HashWidth::W8) && value_width != Some(HashWidth::W8);
                if fits && narrow {
                    return Some(());
                }

                match (fits, resolved) {
                    (true, None) => resolved = Some(self.wide.len()),
                    (true, Some(_)) => {
                        self.ambiguous = true;
                        return None;
                    }
                    (false, _) => {}
                }
                // `resolved` is the record length after the valid combination. The truncation
                // removes the offsets of every other combination.
                self.wide.truncate(resolved.unwrap_or(mark));
            }
        }
        resolved.map(|_| ())
    }

    fn entries_with(
        &mut self,
        key: MapSide,
        value: MapSide,
        count: usize,
        pos: usize,
        end: usize,
    ) -> Option<()> {
        if let (Some(key_width), Some(value_width)) = (key.fixed_width(), value.fixed_width()) {
            let pair = key_width + value_width;
            if count.checked_mul(pair) != Some(end - pos) {
                return None;
            }
            let wide_key = key.width == Some(HashWidth::W8);
            let wide_value = value.width == Some(HashWidth::W8);
            if !wide_key && !wide_value {
                return Some(());
            }

            // The record receives at least `count` offsets. Backtracking can repeat this walk.
            self.charge(count)?;
            for index in 0..count {
                let at = pos + index * pair;
                if wide_key {
                    self.wide.push(at);
                }
                if wide_value {
                    self.wide.push(at + key_width);
                }
            }
            return Some(());
        }

        let mut pos = pos;
        for _ in 0..count {
            self.tick()?;
            pos = self.map_side(key, pos, end)?;
            pos = self.map_side(value, pos, end)?;
        }
        (pos == end).then_some(())
    }

    fn map_side(&mut self, side: MapSide, pos: usize, end: usize) -> Option<usize> {
        let Some(width) = side.width else {
            return self.value(side.kind, pos, end);
        };
        let next = bounded(pos.checked_add(width.bytes())?, end)?;
        if width == HashWidth::W8 {
            self.wide.push(pos);
        }
        Some(next)
    }

    /// Walks `count` properties. The properties occupy `start..end`.
    ///
    /// Reads each `Hash` property as 4 bytes first and records a [`Choice`] for it. If the region
    /// does not end at `end`, reads the property of the last [`Choice`] as 8 bytes and continues
    /// from that property.
    fn properties(&mut self, start: usize, end: usize, count: u16) -> Option<()> {
        let floor = self.choices.len();
        let result = self.properties_above(floor, start, end, count);
        self.choices.truncate(floor);
        result
    }

    fn properties_above(
        &mut self,
        floor: usize,
        start: usize,
        end: usize,
        count: u16,
    ) -> Option<()> {
        let (mut index, mut pos) = (0, start);
        loop {
            while index < count {
                match self.property(index, pos, end) {
                    Some(next) => {
                        pos = next;
                        index += 1;
                    }
                    None => break,
                }
            }
            if index == count && pos == end {
                return Some(());
            }

            loop {
                if self.choices.len() == floor {
                    return None;
                }
                let choice = self.choices.pop()?;
                self.tick()?;
                self.wide.truncate(choice.mark);

                if let Some(next) = choice.at.checked_add(8).and_then(|next| bounded(next, end)) {
                    self.wide.push(choice.at);
                    index = choice.index + 1;
                    pos = next;
                    break;
                }
            }
        }
    }

    /// Walks the property at `pos`. Returns the offset after the property.
    fn property(&mut self, index: u16, pos: usize, end: usize) -> Option<usize> {
        self.tick()?;
        let kind = self.kind(pos.checked_add(4)?, end)?;
        let mut at = pos + 5;

        match kind {
            Kind::Hash => {}
            Kind::Optional => {
                let item_kind = self.item_kind(at, end)?;
                let present = self.u8(at + 1, end)? != 0;
                at += 2;
                if !present {
                    return Some(at);
                }
                if item_kind != Kind::Hash {
                    return self.value(item_kind, at, end);
                }
            }
            kind => return self.value(kind, at, end),
        }

        let next = bounded(at.checked_add(4)?, end)?;
        self.choices.push(Choice {
            index,
            at,
            mark: self.wide.len(),
        });
        Some(next)
    }
}

/// The key side or the value side of a map. `width` is the width that the search tries if the
/// side is a `Hash`.
#[derive(Clone, Copy)]
struct MapSide {
    kind: Kind,
    width: Option<HashWidth>,
}

impl MapSide {
    fn fixed_width(self) -> Option<usize> {
        match self.width {
            Some(width) => Some(width.bytes()),
            None => self.kind.fixed_width(),
        }
    }
}

/// Returns `pos` if `pos` is not after `end`. Returns `None` otherwise.
fn bounded(pos: usize, end: usize) -> Option<usize> {
    (pos <= end).then_some(pos)
}
