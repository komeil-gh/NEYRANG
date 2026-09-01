use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::chess::Move;

use super::{MAX_PLY, VALUE_MATE};

const MATE_THRESHOLD: i32 = VALUE_MATE - MAX_PLY as i32;
const EMPTY_DEPTH: i16 = i16::MIN;
const SHARED_ENTRY_BYTES: usize = size_of::<AtomicU64>();
const SIGNATURE_SHIFT: u32 = 48;
const MOVE_SHIFT: u32 = 32;
const SCORE_SHIFT: u32 = 16;
const DEPTH_SHIFT: u32 = 8;
const GENERATION_SHIFT: u32 = 2;
const GENERATION_MASK: u8 = 0x3f;

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
    storage: Storage,
    mask: usize,
    generation: u8,
}

enum Storage {
    Local(Vec<Entry>),
    Shared(Arc<[AtomicU64]>),
}

impl TranspositionTable {
    pub fn new(megabytes: usize) -> Self {
        let bytes = megabytes.max(1).saturating_mul(1024 * 1024);
        let requested = (bytes / size_of::<Entry>()).max(1);
        let count = floor_power_of_two(requested);
        Self {
            storage: Storage::Local(vec![Entry::EMPTY; count]),
            mask: count - 1,
            generation: 1,
        }
    }

    pub fn new_shared(megabytes: usize) -> Self {
        let bytes = megabytes.max(1).saturating_mul(1024 * 1024);
        let requested = (bytes / SHARED_ENTRY_BYTES).max(1);
        let count = floor_power_of_two(requested);
        let entries = (0..count)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>()
            .into();
        Self {
            storage: Storage::Shared(entries),
            mask: count - 1,
            generation: 1,
        }
    }

    pub fn is_shared(&self) -> bool {
        matches!(&self.storage, Storage::Shared(_))
    }

    pub fn allocated_bytes(&self) -> usize {
        match &self.storage {
            Storage::Local(entries) => entries.len().saturating_mul(size_of::<Entry>()),
            Storage::Shared(entries) => entries.len().saturating_mul(SHARED_ENTRY_BYTES),
        }
    }

    pub fn shared_handle(&self) -> Self {
        let Storage::Shared(entries) = &self.storage else {
            panic!("a local transposition table cannot be shared");
        };
        Self {
            storage: Storage::Shared(Arc::clone(entries)),
            mask: self.mask,
            generation: self.generation,
        }
    }

    pub fn clear(&mut self) {
        match &mut self.storage {
            Storage::Local(entries) => entries.fill(Entry::EMPTY),
            Storage::Shared(entries) => {
                for entry in entries.iter() {
                    entry.store(0, Ordering::Relaxed);
                }
            }
        }
        self.generation = 1;
    }

    pub fn new_search(&mut self) {
        match &self.storage {
            Storage::Local(_) => {
                self.generation = self.generation.wrapping_add(1);
                if self.generation == 0 {
                    self.clear();
                }
            }
            Storage::Shared(_) => {
                self.generation = self.generation.wrapping_add(1) & GENERATION_MASK;
                if self.generation == 0 {
                    self.clear();
                }
            }
        }
    }

    pub fn probe(&self, key: u64, ply: usize) -> Option<TtData> {
        match &self.storage {
            Storage::Local(entries) => {
                let entry = entries[(key as usize) & self.mask];
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
            Storage::Shared(entries) => {
                let packed = entries[(key as usize) & self.mask].load(Ordering::Relaxed);
                unpack_shared(packed, key, ply)
            }
        }
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
        match &mut self.storage {
            Storage::Local(entries) => {
                let current = entries[index];
                let replace = current.depth == EMPTY_DEPTH
                    || current.key == key
                    || current.generation != self.generation
                    || depth + i16::from(bound == Bound::Exact) * 2 >= current.depth;
                if !replace {
                    return;
                }
                entries[index] = Entry {
                    key,
                    best_move,
                    score: normalized_score(score, ply),
                    depth,
                    bound,
                    generation: self.generation,
                };
            }
            Storage::Shared(entries) => {
                let entry = &entries[index];
                let desired =
                    pack_shared(key, depth, score, bound, best_move, ply, self.generation);
                let mut observed = entry.load(Ordering::Relaxed);
                loop {
                    if !should_replace_shared(observed, key, depth, bound, self.generation) {
                        return;
                    }
                    match entry.compare_exchange_weak(
                        observed,
                        desired,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => return,
                        Err(actual) => observed = actual,
                    }
                }
            }
        }
    }

    /// Approximate permille occupancy for entries touched by this search.
    pub fn hashfull(&self) -> u16 {
        let len = match &self.storage {
            Storage::Local(entries) => entries.len(),
            Storage::Shared(entries) => entries.len(),
        };
        let sample = len.min(1_000);
        if sample == 0 {
            return 0;
        }
        let used = match &self.storage {
            Storage::Local(entries) => entries[..sample]
                .iter()
                .filter(|entry| entry.depth != EMPTY_DEPTH && entry.generation == self.generation)
                .count(),
            Storage::Shared(entries) => entries[..sample]
                .iter()
                .filter(|entry| {
                    let packed = entry.load(Ordering::Relaxed);
                    packed != 0 && packed_generation(packed) == self.generation
                })
                .count(),
        };
        (used * 1_000 / sample) as u16
    }
}

fn normalized_score(score: i32, ply: usize) -> i16 {
    score_to_tt(score, ply).clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16
}

fn key_signature(key: u64) -> u16 {
    (key >> SIGNATURE_SHIFT) as u16
}

fn pack_shared(
    key: u64,
    depth: i16,
    score: i32,
    bound: Bound,
    best_move: Move,
    ply: usize,
    generation: u8,
) -> u64 {
    let depth = depth.clamp(i8::MIN as i16, i8::MAX as i16) as i8 as u8;
    u64::from(key_signature(key)) << SIGNATURE_SHIFT
        | u64::from(best_move.raw()) << MOVE_SHIFT
        | u64::from(normalized_score(score, ply) as u16) << SCORE_SHIFT
        | u64::from(depth) << DEPTH_SHIFT
        | u64::from(generation & GENERATION_MASK) << GENERATION_SHIFT
        | bound as u64
}

fn unpack_shared(packed: u64, key: u64, ply: usize) -> Option<TtData> {
    if packed == 0 || (packed >> SIGNATURE_SHIFT) as u16 != key_signature(key) {
        return None;
    }
    let bound = match (packed & 0x3) as u8 {
        0 => Bound::Exact,
        1 => Bound::Lower,
        2 => Bound::Upper,
        _ => return None,
    };
    let best_move = Move::from_raw(((packed >> MOVE_SHIFT) & u64::from(u16::MAX)) as u16);
    let score = ((packed >> SCORE_SHIFT) & u64::from(u16::MAX)) as u16 as i16;
    let depth = ((packed >> DEPTH_SHIFT) & u64::from(u8::MAX)) as u8 as i8;
    Some(TtData {
        best_move,
        score: score_from_tt(score as i32, ply),
        depth: depth as i16,
        bound,
    })
}

fn packed_generation(packed: u64) -> u8 {
    ((packed >> GENERATION_SHIFT) as u8) & GENERATION_MASK
}

fn should_replace_shared(packed: u64, key: u64, depth: i16, bound: Bound, generation: u8) -> bool {
    if packed == 0 {
        return true;
    }
    let current_signature = (packed >> SIGNATURE_SHIFT) as u16;
    let current_depth = ((packed >> DEPTH_SHIFT) & u64::from(u8::MAX)) as u8 as i8 as i16;
    current_signature == key_signature(key)
        || packed_generation(packed) != generation
        || depth + i16::from(bound == Bound::Exact) * 2 >= current_depth
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
