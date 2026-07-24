use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use crate::{
    chess::Position,
    search::{SearchLimits, Searcher, tt::TranspositionTable},
};

const POSITIONS: [&str; 5] = [
    Position::STARTPOS_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
];

#[derive(Clone, Copy, Debug)]
pub struct BenchmarkResult {
    pub positions: usize,
    pub nodes: u64,
    pub elapsed: Duration,
    pub checksum: u64,
}

impl BenchmarkResult {
    pub fn nps(self) -> u64 {
        let micros = self.elapsed.as_micros().max(1) as u64;
        self.nodes.saturating_mul(1_000_000) / micros
    }
}

pub fn run(depth: u8) -> Result<BenchmarkResult, String> {
    let mut table = TranspositionTable::new(16);
    let started = Instant::now();
    let mut nodes = 0_u64;
    let mut checksum = 0xCBF2_9CE4_8422_2325_u64;
    for fen in POSITIONS {
        let mut position = Position::from_fen(fen).map_err(|error| error.to_string())?;
        let hashes = [position.hash()];
        let stop = AtomicBool::new(false);
        let mut searcher = Searcher::with_table(&stop, table);
        let result = searcher.search(&mut position, &SearchLimits::depth(depth), &hashes, |_| {});
        nodes = nodes.saturating_add(result.nodes);
        let best = result.best_move.map_or(0, |mv| u64::from(mv.raw()));
        checksum ^= position.hash();
        checksum = checksum.wrapping_mul(0x0000_0100_0000_01B3);
        checksum ^=
            result.nodes ^ best.rotate_left(17) ^ (result.score as i64 as u64).rotate_left(31);
        checksum = checksum.wrapping_mul(0x0000_0100_0000_01B3);
        table = searcher.into_table();
    }
    Ok(BenchmarkResult {
        positions: POSITIONS.len(),
        nodes,
        elapsed: started.elapsed(),
        checksum,
    })
}
