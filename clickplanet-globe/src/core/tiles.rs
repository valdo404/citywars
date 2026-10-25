//! Tile ownership state shared by the game logic and the globe renderer.
//!
//! Tile ids are 1-based, as in the original Clickplanet front end and the server: tile `id` is the
//! vertex `id - 1` of `coordinates.json`. Owners are stored as indices in a [`CountryTable`]; index 0
//! means "no owner".

use std::collections::HashMap;
use std::ops::Range;

/// Maps country codes to small indices used by the GPU (index 0 is reserved for "no owner").
#[derive(Debug, Clone, Default)]
pub struct CountryTable {
    codes: Vec<String>,
    index: HashMap<String, u32>,
}

impl CountryTable {
    /// Builds the table from the codes in a stable order (index = position + 1).
    pub fn new<I: IntoIterator<Item = String>>(codes: I) -> Self {
        let mut table = CountryTable::default();
        table.codes.push(String::new());
        for code in codes {
            if !table.index.contains_key(&code) {
                table.index.insert(code.clone(), table.codes.len() as u32);
                table.codes.push(code);
            }
        }
        table
    }

    pub fn index_of(&self, code: &str) -> Option<u32> {
        self.index.get(code).copied()
    }

    /// The country code for an index, `None` for 0 (no owner) or an unknown index.
    pub fn code_of(&self, index: u32) -> Option<&str> {
        if index == 0 {
            return None;
        }
        self.codes.get(index as usize).map(String::as_str)
    }

    /// Number of entries including the reserved "no owner" entry.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.len() <= 1
    }
}

/// What has to be uploaded to the GPU instance buffer since the last call to [`TileState::take_dirty`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirtyUpload {
    None,
    /// Ranges of tile indices (0-based, end exclusive), sorted and disjoint.
    Ranges(Vec<Range<u32>>),
}

/// Owner of every tile, plus the set of tiles changed since the last upload.
#[derive(Debug, Clone)]
pub struct TileState {
    owners: Vec<u32>,
    dirty: Vec<u32>,
}

/// Two dirty tiles closer than this are uploaded in a single write.
const MERGE_GAP: u32 = 32;
/// Above this many separate ranges, a single span is uploaded instead.
const MAX_RANGES: usize = 256;

impl TileState {
    pub fn new(tile_count: usize) -> Self {
        TileState {
            owners: vec![0; tile_count],
            dirty: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.owners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    /// Owner index of a tile (1-based id), 0 when unowned or out of range.
    pub fn owner(&self, tile_id: u32) -> u32 {
        tile_id
            .checked_sub(1)
            .and_then(|i| self.owners.get(i as usize))
            .copied()
            .unwrap_or(0)
    }

    /// Sets the owner of a tile (1-based id). Returns the previous owner, or `None` when the id is
    /// out of range (the update is then ignored).
    pub fn set_owner(&mut self, tile_id: u32, owner: u32) -> Option<u32> {
        let index = tile_id.checked_sub(1)?;
        let slot = self.owners.get_mut(index as usize)?;
        let previous = *slot;
        if previous != owner {
            *slot = owner;
            self.dirty.push(index);
        }
        Some(previous)
    }

    /// Owner indices, indexed by `tile_id - 1` (the GPU instance order).
    pub fn owners(&self) -> &[u32] {
        &self.owners
    }

    /// Returns the ranges to upload and clears the dirty set.
    pub fn take_dirty(&mut self) -> DirtyUpload {
        if self.dirty.is_empty() {
            return DirtyUpload::None;
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.sort_unstable();
        dirty.dedup();

        let mut ranges: Vec<Range<u32>> = Vec::new();
        for index in dirty {
            match ranges.last_mut() {
                Some(last) if index <= last.end + MERGE_GAP => last.end = index + 1,
                _ => ranges.push(index..index + 1),
            }
        }
        if ranges.len() > MAX_RANGES {
            let start = ranges.first().map(|r| r.start).unwrap_or(0);
            let end = ranges.last().map(|r| r.end).unwrap_or(0);
            ranges.clear();
            ranges.push(start..end);
        }
        DirtyUpload::Ranges(ranges)
    }

    /// Number of tiles owned by each country index (index 0 = unowned tiles).
    pub fn counts(&self, country_count: usize) -> Vec<u32> {
        let mut counts = vec![0u32; country_count.max(1)];
        for &owner in &self.owners {
            if let Some(count) = counts.get_mut(owner as usize) {
                *count += 1;
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> CountryTable {
        CountryTable::new(["fr", "de", "us"].iter().map(|s| s.to_string()))
    }

    #[test]
    fn country_table_reserves_index_zero() {
        let t = table();
        assert_eq!(t.index_of("fr"), Some(1));
        assert_eq!(t.index_of("us"), Some(3));
        assert_eq!(t.code_of(0), None);
        assert_eq!(t.code_of(2), Some("de"));
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn tile_ids_are_one_based() {
        let mut s = TileState::new(3);
        assert_eq!(s.set_owner(0, 1), None);
        assert_eq!(s.set_owner(4, 1), None);
        assert_eq!(s.set_owner(1, 2), Some(0));
        assert_eq!(s.set_owner(1, 3), Some(2));
        assert_eq!(s.owner(1), 3);
        assert_eq!(s.owners(), &[3, 0, 0]);
    }

    #[test]
    fn dirty_ranges_are_merged() {
        let mut s = TileState::new(10_000);
        for id in [5, 3, 4, 1000, 4] {
            s.set_owner(id, 1);
        }
        assert_eq!(s.take_dirty(), DirtyUpload::Ranges(vec![2..5, 999..1000]));
        assert_eq!(s.take_dirty(), DirtyUpload::None);
        // setting the same owner again is not a change
        s.set_owner(5, 1);
        assert_eq!(s.take_dirty(), DirtyUpload::None);
    }

    #[test]
    fn many_ranges_collapse_into_one_span() {
        let mut s = TileState::new(100_000);
        for i in 0..1000u32 {
            s.set_owner(1 + i * 50, 1);
        }
        assert_eq!(s.take_dirty(), DirtyUpload::Ranges(vec![0..49_951]));
    }

    #[test]
    fn counts_per_country() {
        let mut s = TileState::new(4);
        s.set_owner(1, 1);
        s.set_owner(2, 1);
        s.set_owner(3, 2);
        assert_eq!(s.counts(3), vec![1, 2, 1]);
    }
}
