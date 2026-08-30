//! Batch object lookup: one request, one schedule, forward seeks only.

use std::{collections::HashSet, io};

use ltk_hash::BinHash;

use crate::{
    property::NoMeta,
    stream::{BinStream, ObjectEntry, ObjectStream},
    Error,
};

/// Lending cursor over a requested set of objects, in file order.
///
/// Created by [`BinStream::objects_batch`]. Like [`Objects`](crate::stream::Objects), each
/// yielded [`ObjectStream`] borrows the reader, so the borrow checker enforces one open object
/// at a time.
#[must_use = "cursors are lazy and read nothing until advanced"]
#[derive(Debug)]
pub struct BatchObjects<'a, R: io::Read + io::Seek, M = NoMeta> {
    stream: &'a mut BinStream<R, M>,
    /// The request, deduplicated, in the order it was given — the order [`Self::missing`]
    /// reports in.
    requested: Vec<BinHash>,
    /// What the cursor has not found yet.
    pending: HashSet<BinHash>,
    missing: Vec<BinHash>,
    /// The requested rows in offset order, when the table of contents was already complete.
    /// `None` means the cursor is scanning the object table instead.
    ordered: Option<Vec<ObjectEntry>>,
    /// A table position while scanning, an index into `ordered` otherwise.
    at: usize,
}

impl<'a, R: io::Read + io::Seek, M: Default> BatchObjects<'a, R, M> {
    pub(crate) fn new(
        stream: &'a mut BinStream<R, M>,
        hashes: impl IntoIterator<Item = impl Into<BinHash>>,
    ) -> Self {
        let mut requested = Vec::new();
        let mut pending = HashSet::new();
        for hash in hashes {
            let hash = hash.into();
            if pending.insert(hash) {
                requested.push(hash);
            }
        }

        // A complete table of contents answers the whole request without reading anything, so
        // the schedule is decided here: the found rows sorted by offset, the rest missing.
        let (ordered, missing) = match stream.is_toc_complete() {
            false => (None, Vec::new()),
            true => {
                let mut found = Vec::with_capacity(requested.len());
                let mut missing = Vec::new();
                for &hash in &requested {
                    match stream.toc_entry(hash) {
                        Some(&entry) => found.push(entry),
                        None => missing.push(hash),
                    }
                }
                found.sort_unstable_by_key(|entry| entry.offset);
                pending.clear();
                (Some(found), missing)
            }
        };

        Self {
            stream,
            requested,
            pending,
            missing,
            ordered,
            at: 0,
        }
    }

    /// Advances to the next requested object the table contains.
    ///
    /// Absent hashes are not yielded — a miss has no file position, so it has no place in a
    /// file-order sequence. [`BatchObjects::missing`] reports them once the cursor is done.
    ///
    /// # Errors
    ///
    /// An I/O error from the source while scanning the object table.
    #[expect(
        clippy::should_implement_trait,
        reason = "a lending cursor: the yielded item borrows the reader, which `Iterator` cannot express"
    )]
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error> {
        match self.next_entry()? {
            Some(entry) => Ok(Some(ObjectStream::new(self.stream, entry))),
            None => Ok(None),
        }
    }

    /// The requested hashes the object table does not contain.
    ///
    /// Complete once [`BatchObjects::next`] has returned `Ok(None)`; before that it only holds
    /// what the cursor has already ruled out, which for a scan is nothing.
    #[must_use]
    pub fn missing(&self) -> &[BinHash] {
        &self.missing
    }

    /// The next row to open, by whichever schedule this cursor is on.
    fn next_entry(&mut self) -> Result<Option<ObjectEntry>, Error> {
        if let Some(ordered) = &self.ordered {
            let entry = ordered.get(self.at).copied();
            if entry.is_some() {
                self.at += 1;
            }
            return Ok(entry);
        }

        let total = self.stream.class_hashes().len();
        // Nothing left to look for — an empty request, or one the scan already answered — so
        // there is no reason to read any more of the table.
        if self.pending.is_empty() {
            self.at = total;
        }

        while self.at < total {
            let entry = match self.stream.toc_row(self.at) {
                Some(&entry) => entry,
                None => match self.stream.harvest(self.at) {
                    Ok(entry) => entry,
                    Err(error) => {
                        self.at = total;
                        return Err(error);
                    }
                },
            };
            self.at += 1;

            if self.pending.remove(&entry.path_hash) {
                // Nothing left to look for, so the rest of the table is never read.
                if self.pending.is_empty() {
                    self.at = total;
                }
                return Ok(Some(entry));
            }
        }

        let pending = &self.pending;
        let missing: Vec<_> = self
            .requested
            .iter()
            .copied()
            .filter(|hash| pending.contains(hash))
            .collect();
        self.missing = missing;

        Ok(None)
    }
}
