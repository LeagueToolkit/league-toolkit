/// A span of text in the source file - `[start, end)` in bytes.
/// `end` marks the offset after the last byte of the span
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[must_use]
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Whether this span contains `offset`
    #[must_use]
    #[inline]
    pub fn contains(&self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }

    /// Whether two span ranges intersect
    #[must_use]
    #[inline]
    pub fn intersects(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// The length of the span in bytes
    #[must_use]
    #[inline]
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Whether the span is empty
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

impl std::ops::Index<Span> for str {
    type Output = str;

    fn index(&self, index: Span) -> &Self::Output {
        &self[&index]
    }
}
impl std::ops::Index<&Span> for str {
    type Output = str;

    fn index(&self, index: &Span) -> &Self::Output {
        let start = index.start as usize;
        let end = index.end as usize;
        &self[start..end.min(self.len())]
    }
}

impl std::ops::Index<Span> for String {
    type Output = str;

    fn index(&self, index: Span) -> &Self::Output {
        &self.as_str()[index]
    }
}
impl std::ops::Index<&Span> for String {
    type Output = str;

    fn index(&self, index: &Span) -> &Self::Output {
        &self[*index]
    }
}

#[cfg(test)]
mod test {
    use super::Span;

    #[test]
    fn contains_is_half_open() {
        let span = Span::new(2, 5);
        assert!(!span.contains(1));
        assert!(span.contains(2));
        assert!(span.contains(4));
        assert!(!span.contains(5));
        assert!(!span.contains(6));
    }

    #[test]
    fn empty_span_contains_nothing() {
        let span = Span::new(3, 3);
        assert!(!span.contains(2));
        assert!(!span.contains(3));
        assert!(!span.contains(4));
    }

    #[test]
    fn boundary_offset_belongs_to_exactly_one_neighbor() {
        let left = Span::new(0, 3);
        let right = Span::new(3, 6);
        assert!(!left.contains(3));
        assert!(right.contains(3));
        assert!(!left.intersects(&right));
    }

    #[test]
    fn contains_matches_intersects() {
        let span = Span::new(2, 5);
        for offset in 0..8 {
            assert_eq!(
                span.contains(offset),
                span.intersects(&Span::new(offset, offset + 1)),
                "offset {offset}"
            );
        }
    }
}
