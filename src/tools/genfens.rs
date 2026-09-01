//! Deterministic, OpenBench-compatible opening generation outside the UCI hot path.

use std::{error::Error, fmt, io::Write};

use crate::{
    chess::{Move, Position},
    rekhne::VALUE_MATE,
    sanj,
};

const DEFAULT_MIN_PLIES: u8 = 8;
const DEFAULT_MAX_PLIES: u8 = 20;
const DEFAULT_SCORE_MARGIN_CP: i32 = 100;
const DEFAULT_MAX_ABS_EVAL_CP: i32 = 180;
const MAX_OPENINGS_PER_INVOCATION: usize = 1_000_000;
const MAX_ATTEMPTS_PER_OPENING: u64 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Config {
    count: usize,
    seed: u64,
    min_plies: u8,
    max_plies: u8,
    score_margin_cp: i32,
    max_abs_eval_cp: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationSummary {
    pub generated: usize,
    pub seed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenfensError(String);

impl GenfensError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for GenfensError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for GenfensError {}

/// Parse and execute one complete OpenBench `genfens ...` command line.
///
/// Each opening is derived only from `seed + index`, so splitting a workload at
/// seed offsets produces the same byte-for-byte sequence as one larger call.
pub fn run<W: Write>(
    command_line: &str,
    output: &mut W,
) -> Result<GenerationSummary, GenfensError> {
    let config = Config::parse(command_line)?;

    for index in 0..config.count {
        let opening_seed = config.seed.wrapping_add(index as u64);
        let position = generate_opening(opening_seed, config)?;
        writeln!(output, "info string genfens {}", position.to_fen())
            .map_err(|error| GenfensError::new(format!("cannot write genfens output: {error}")))?;
        output
            .flush()
            .map_err(|error| GenfensError::new(format!("cannot flush genfens output: {error}")))?;
    }

    Ok(GenerationSummary {
        generated: config.count,
        seed: config.seed,
    })
}

impl Config {
    fn parse(command_line: &str) -> Result<Self, GenfensError> {
        let tokens = command_line.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.first().copied() != Some("genfens") {
            return Err(GenfensError::new("command must start with 'genfens'"));
        }
        if tokens.len() < 6 {
            return Err(GenfensError::new(
                "usage: genfens N seed S book None [minplies P] [maxplies P] [margin CP] [maxeval CP]",
            ));
        }

        let count = parse_usize(tokens[1], "opening count")?;
        if count == 0 || count > MAX_OPENINGS_PER_INVOCATION {
            return Err(GenfensError::new(format!(
                "opening count must be between 1 and {MAX_OPENINGS_PER_INVOCATION}"
            )));
        }
        if tokens[2] != "seed" {
            return Err(GenfensError::new(
                "opening count must be followed by 'seed S'",
            ));
        }
        let seed = tokens[3]
            .parse::<u64>()
            .map_err(|_| GenfensError::new("seed must be an unsigned 64-bit integer"))?;
        if tokens[4] != "book" {
            return Err(GenfensError::new("seed must be followed by 'book None'"));
        }
        if tokens[5] != "None" {
            return Err(GenfensError::new(
                "this generator version supports only 'book None'",
            ));
        }

        let mut config = Self {
            count,
            seed,
            min_plies: DEFAULT_MIN_PLIES,
            max_plies: DEFAULT_MAX_PLIES,
            score_margin_cp: DEFAULT_SCORE_MARGIN_CP,
            max_abs_eval_cp: DEFAULT_MAX_ABS_EVAL_CP,
        };
        let mut index = 6;
        while index < tokens.len() {
            if index + 1 >= tokens.len() {
                return Err(GenfensError::new(format!(
                    "genfens option '{}' requires a value",
                    tokens[index]
                )));
            }
            let value = tokens[index + 1];
            match tokens[index] {
                "minplies" => config.min_plies = parse_u8(value, "minplies")?,
                "maxplies" => config.max_plies = parse_u8(value, "maxplies")?,
                "margin" => config.score_margin_cp = parse_i32(value, "margin")?,
                "maxeval" => config.max_abs_eval_cp = parse_i32(value, "maxeval")?,
                option => {
                    return Err(GenfensError::new(format!(
                        "unknown genfens option '{option}'"
                    )));
                }
            }
            index += 2;
        }
        config.validate()?;
        Ok(config)
    }

    fn validate(self) -> Result<(), GenfensError> {
        if self.min_plies < 2
            || self.max_plies > 60
            || self.min_plies > self.max_plies
            || !self.min_plies.is_multiple_of(2)
            || !self.max_plies.is_multiple_of(2)
        {
            return Err(GenfensError::new(
                "minplies and maxplies must be even, ordered, and within 2..=60",
            ));
        }
        if !(0..=500).contains(&self.score_margin_cp) {
            return Err(GenfensError::new("margin must be between 0 and 500 cp"));
        }
        if !(0..=2_000).contains(&self.max_abs_eval_cp) {
            return Err(GenfensError::new("maxeval must be between 0 and 2000 cp"));
        }
        Ok(())
    }
}

fn generate_opening(seed: u64, config: Config) -> Result<Position, GenfensError> {
    for attempt in 0..MAX_ATTEMPTS_PER_OPENING {
        let attempt_seed =
            seed.wrapping_add(attempt.wrapping_mul(0xd134_2543_de82_ef95)) ^ 0xa076_1d64_78bd_642f;
        let mut rng = SplitMix64::new(attempt_seed);
        let mut position = Position::startpos();
        let spans = usize::from((config.max_plies - config.min_plies) / 2) + 1;
        let target_plies = config.min_plies + 2 * rng.index(spans) as u8;
        let mut complete = true;

        for _ in 0..target_plies {
            let Some(mv) = choose_move(&mut position, &mut rng, config.score_margin_cp) else {
                complete = false;
                break;
            };
            position.make_move(mv);
        }

        if complete
            && !position.is_in_check(position.side_to_move())
            && !position.legal_moves().is_empty()
            && sanj::evaluate(&position).abs() <= config.max_abs_eval_cp
        {
            return Ok(position);
        }
    }

    Err(GenfensError::new(format!(
        "could not generate an accepted opening for seed {seed} after {MAX_ATTEMPTS_PER_OPENING} attempts"
    )))
}

fn choose_move(position: &mut Position, rng: &mut SplitMix64, margin: i32) -> Option<Move> {
    let legal_moves = position.legal_moves();
    let mut scored = Vec::with_capacity(legal_moves.len());
    let mut best = -VALUE_MATE;

    for &mv in legal_moves.iter() {
        let undo = position.make_move(mv);
        let replies = position.legal_moves();
        if replies.is_empty() {
            position.unmake_move(mv, undo);
            continue;
        }

        let mut worst_reply = VALUE_MATE;
        for &reply in replies.iter() {
            let reply_undo = position.make_move(reply);
            let continuations = position.legal_moves();
            let score = if continuations.is_empty() {
                if position.is_in_check(position.side_to_move()) {
                    -VALUE_MATE
                } else {
                    0
                }
            } else {
                sanj::evaluate(position)
            };
            position.unmake_move(reply, reply_undo);
            worst_reply = worst_reply.min(score);
        }
        position.unmake_move(mv, undo);
        best = best.max(worst_reply);
        scored.push((mv, worst_reply));
    }

    let candidates = scored
        .iter()
        .filter(|(_, score)| *score >= best - margin)
        .map(|(mv, _)| *mv)
        .collect::<Vec<_>>();
    candidates.get(rng.index(candidates.len())).copied()
}

fn parse_usize(value: &str, name: &str) -> Result<usize, GenfensError> {
    value
        .parse()
        .map_err(|_| GenfensError::new(format!("{name} must be an unsigned integer")))
}

fn parse_u8(value: &str, name: &str) -> Result<u8, GenfensError> {
    value
        .parse()
        .map_err(|_| GenfensError::new(format!("{name} must be an unsigned 8-bit integer")))
}

fn parse_i32(value: &str, name: &str) -> Result<i32, GenfensError> {
    value
        .parse()
        .map_err(|_| GenfensError::new(format!("{name} must be a signed 32-bit integer")))
}

#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        let bound = upper as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return (value % bound) as usize;
            }
        }
    }
}
