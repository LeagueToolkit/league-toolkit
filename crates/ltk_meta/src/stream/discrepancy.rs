//! Size discrepancies: declared-versus-consumed disagreements, reported instead of raised.
//!
//! The stream takes the client's strictness: counts drive the parse, sizes are only trusted as
//! skip distances, and a size that disagrees with what parsing consumed is recorded here rather
//! than failing the read. Survey tooling reads the log after a sweep; strict consumers treat a
//! non-zero count as an error.

use std::fmt;

/// A region whose declared size did not match what parsing consumed.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeDiscrepancy {
    /// Absolute offset of the region's `u32` size field in the stream.
    pub offset: u64,
    /// The size the file declares for the region.
    pub declared: u64,
    /// The bytes the count-driven walk actually consumed.
    pub consumed: u64,
}

impl fmt::Display for SizeDiscrepancy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "region at {:#x} declares {} bytes, parsing consumed {}",
            self.offset, self.declared, self.consumed
        )
    }
}

/// A bounded record of the [`SizeDiscrepancy`]s a walk observed.
///
/// The first [`DiscrepancyLog::RETAINED`] discrepancies are kept; [`DiscrepancyLog::total`]
/// keeps counting past the cap, so a hostile file cannot grow memory through its own
/// corruption. Empty on a well-formed file.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct DiscrepancyLog {
    retained: Vec<SizeDiscrepancy>,
    total: u64,
}

impl DiscrepancyLog {
    /// How many discrepancies are retained before recording becomes counting only.
    pub const RETAINED: usize = 64;

    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one discrepancy, retaining it if the cap has room.
    pub fn record(&mut self, discrepancy: SizeDiscrepancy) {
        if self.retained.len() < Self::RETAINED {
            self.retained.push(discrepancy);
        }
        self.total += 1;
    }

    /// The retained discrepancies, in the order they were observed.
    #[must_use]
    pub fn retained(&self) -> &[SizeDiscrepancy] {
        &self.retained
    }

    /// Total discrepancies observed, including those past the retention cap.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Whether no discrepancy has been observed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

impl fmt::Display for DiscrepancyLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} size discrepancies ({} retained)",
            self.total,
            self.retained.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_the_first_64_and_counts_the_rest() {
        let mut log = DiscrepancyLog::new();
        assert!(log.is_empty());

        for i in 0..100u64 {
            log.record(SizeDiscrepancy {
                offset: i,
                declared: 10,
                consumed: 12,
            });
        }

        assert_eq!(log.retained().len(), DiscrepancyLog::RETAINED);
        assert_eq!(log.total(), 100);
        assert_eq!(log.retained()[63].offset, 63);
        assert!(!log.is_empty());
        assert_eq!(log.to_string(), "100 size discrepancies (64 retained)");
    }
}
