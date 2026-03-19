/// A span in the source text (offset and length).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default)]
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

    #[must_use]
    #[inline]
    pub fn contains(&self, offset: u32) -> bool {
        self.start <= offset && offset <= self.end
    }

    #[must_use]
    #[inline]
    pub fn intersects(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    #[must_use]
    #[inline]
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

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
