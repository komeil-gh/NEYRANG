use std::mem::size_of;

use crate::chess::Move;

use super::{MAX_PLY, VALUE_MATE};

const MATE_THRESHOLD: i32 = VALUE_MATE - MAX_PLY as i32;
const EMPTY_DEPTH: i16 = i16::MIN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TtData {
    pub best_move: Move,
    pub score: i32,
    pub depth: i16,
    pub bound: Bound,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Entry {
    key: u64,
    best_move: Move,
    score: i16,
    depth: i16,
    bound: Bound,
    generation: u8,
}

impl Entry {
    const EMPTY: Self = Self {
        key: 0,
        best_move: Move::NONE,
        score: 0,
        depth: EMPTY_DEPTH,
        bound: Bound::Upper,
        generation: 0,
    };
}

/// One-entry-per-index table. The layout is intentionally simple until cluster
/// replacement is justified by profiling and match data.
pub struct TranspositionTable {
    entries: Vec<Entry>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    pub fn new(megabytes: usize) -> Self {
        let bytes = megabytes.max(1).saturating_mul(1024 * 1024);
        let requested = (bytes / size_of::<Entry>()).max(1);
        let count = floor_power_of_two(requested);
        Self {
            entries: vec![Entry::EMPTY; count],
            mask: count - 1,
            generation: 1,
        }
    }

    pub fn clear(&mut self) {
        self.entries.fill(Entry::EMPTY);
        self.generation = 1;
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.clear();
        }
    }

    pub fn probe(&self, key: u64, ply: usize) -> Option<TtData> {
        let entry = self.entries[(key as usize) & self.mask];
        if entry.depth == EMPTY_DEPTH || entry.key != key {
            return None;
        }
        Some(TtData {
            best_move: entry.best_move,
            score: score_from_tt(entry.score as i32, ply),
            depth: entry.depth,
            bound: entry.bound,
        })
    }

    pub fn store(
        &mut self,
        key: u64,
        depth: i16,
        score: i32,
        bound: Bound,
        best_move: Move,
        ply: usize,
    ) {
        let index = (key as usize) & self.mask;
        let current = self.entries[index];
        let replace = current.depth == EMPTY_DEPTH
            || current.key == key
            || current.generation != self.generation
            || depth + i16::from(bound == Bound::Exact) * 2 >= current.depth;
        if !replace {
            return;
        }
        self.entries[index] = Entry {
            key,
            best_move,
            score: score_to_tt(score, ply).clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16,
            depth,
            bound,
            generation: self.generation,
        };
    }

    /// Approximate permille occupancy for entries touched by this search.
    pub fn hashfull(&self) -> u16 {
        let sample = self.entries.len().min(1_000);
        if sample == 0 {
            return 0;
        }
        let used = self.entries[..sample]
            .iter()
            .filter(|entry| entry.depth != EMPTY_DEPTH && entry.generation == self.generation)
            .count();
        (used * 1_000 / sample) as u16
    }
}

fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score + ply as i32
    } else if score <= -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    }
}

fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score - ply as i32
    } else if score <= -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

fn floor_power_of_two(value: usize) -> usize {
    1_usize << (usize::BITS - 1 - value.leading_zeros())
}
