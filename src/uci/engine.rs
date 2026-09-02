#[cfg(feature = "nnue")]
use std::fs;
use std::{
    io::{self, BufRead, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    chess::{Color, Position, perft},
    engine::info::{ENGINE_AUTHOR, ENGINE_NAME, ENGINE_VERSION},
    rekhne::{
        MAX_PLY, ParallelOptions, SearchInfo, SearchLimits, Searcher, VALUE_MATE,
        search_parallel_with_evaluator, time::TimeManager, tt::TranspositionTable,
    },
    sanj,
};

use super::parser::{Command, GoParameters, PositionSpecification, parse};

const DEFAULT_HASH_MB: usize = 64;
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 30;
const MAX_HASH_MB: usize = 65_536;

pub fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut engine = UciEngine::new();
    for line in stdin.lock().lines() {
        engine.reap_finished();
        let line = line?;
        let command = match parse(&line) {
            Ok(command) => command,
            Err(error) => {
                send_line(&format!("info string error: {error}"))?;
                continue;
            }
        };
        if !engine.handle(command)? {
            break;
        }
    }
    engine.stop_search();
    Ok(())
}

struct UciEngine {
    position: Position,
    game_hashes: Vec<u64>,
    hash_megabytes: usize,
    threads: usize,
    move_overhead_ms: u64,
    active: Option<ActiveSearch>,
    table: Option<TranspositionTable>,
    evaluator: sanj::Evaluator,
}

impl UciEngine {
    fn new() -> Self {
        let mut position = Position::startpos();
        let hash = position.repetition_hash();
        Self {
            position,
            game_hashes: vec![hash],
            hash_megabytes: DEFAULT_HASH_MB,
            threads: 1,
            move_overhead_ms: DEFAULT_MOVE_OVERHEAD_MS,
            active: None,
            table: Some(table_for(DEFAULT_HASH_MB, 1)),
            evaluator: sanj::Evaluator::classical(),
        }
    }

    fn handle(&mut self, command: Command) -> io::Result<bool> {
        match command {
            Command::Uci => self.identify()?,
            Command::IsReady => send_line("readyok")?,
            Command::NewGame => {
                self.stop_search();
                self.position = Position::startpos();
                self.game_hashes.clear();
                self.game_hashes.push(self.position.repetition_hash());
                if let Some(table) = &mut self.table {
                    table.clear();
                }
            }
            Command::Position(specification) => {
                self.stop_search();
                if let Err(error) = self.set_position(specification) {
                    send_line(&format!("info string error: {error}"))?;
                }
            }
            Command::Go(parameters) => {
                self.stop_search();
                self.start_search(parameters);
            }
            Command::Stop => self.stop_search(),
            Command::Quit => {
                self.stop_search();
                return Ok(false);
            }
            Command::SetOption { name, value } => {
                self.stop_search();
                if let Err(error) = self.set_option(&name, value.as_deref()) {
                    send_line(&format!("info string error: {error}"))?;
                }
            }
            Command::Display => {
                send_line(&format!("info string fen {}", self.position.to_fen()))?;
            }
            Command::Evaluate => {
                send_line(&format!(
                    "info string eval {} cp",
                    self.evaluator.evaluate(&self.position)
                ))?;
            }
            Command::Perft(depth) => {
                self.stop_search();
                let mut position = self.position.clone();
                send_line(&format!(
                    "info string perft depth {depth} nodes {}",
                    perft(&mut position, depth)
                ))?;
            }
            Command::Unknown(name) => {
                send_line(&format!("info string unknown command '{name}'"))?;
            }
        }
        Ok(true)
    }

    fn identify(&self) -> io::Result<()> {
        send_line(&format!("id name {ENGINE_NAME} {ENGINE_VERSION}"))?;
        send_line(&format!("id author {ENGINE_AUTHOR}"))?;
        send_line(&format!(
            "option name Hash type spin default {DEFAULT_HASH_MB} min 1 max {MAX_HASH_MB}"
        ))?;
        send_line("option name Threads type spin default 1 min 1 max 256")?;
        send_line(&format!(
            "option name Move Overhead type spin default {DEFAULT_MOVE_OVERHEAD_MS} min 0 max 5000"
        ))?;
        #[cfg(feature = "nnue")]
        send_line("option name EvalFile type string default <empty>")?;
        send_line("uciok")
    }

    fn set_position(&mut self, specification: PositionSpecification) -> Result<(), String> {
        let mut position = match specification.fen {
            Some(fen) => Position::from_fen(&fen).map_err(|error| error.to_string())?,
            None => Position::startpos(),
        };
        let mut hashes = vec![position.repetition_hash()];
        for notation in specification.moves {
            let mv = position
                .find_legal_move(&notation)
                .ok_or_else(|| format!("illegal move '{notation}' in position command"))?;
            position.make_move(mv);
            hashes.push(position.repetition_hash());
        }
        self.position = position;
        self.game_hashes = hashes;
        Ok(())
    }

    fn set_option(&mut self, name: &str, value: Option<&str>) -> Result<(), String> {
        let value = value.ok_or_else(|| format!("option '{name}' requires a value"))?;
        match name.to_ascii_lowercase().as_str() {
            "hash" => {
                let parsed: usize = value
                    .parse()
                    .map_err(|_| "Hash must be an integer".to_owned())?;
                if !(1..=MAX_HASH_MB).contains(&parsed) {
                    return Err(format!("Hash must be between 1 and {MAX_HASH_MB} MB"));
                }
                self.hash_megabytes = parsed;
                self.table = Some(table_for(parsed, self.threads));
            }
            "threads" => {
                let parsed: usize = value
                    .parse()
                    .map_err(|_| "Threads must be an integer".to_owned())?;
                if !(1..=256).contains(&parsed) {
                    return Err("Threads must be between 1 and 256".to_owned());
                }
                let changes_table_mode = (self.threads == 1) != (parsed == 1);
                self.threads = parsed;
                if changes_table_mode {
                    self.table = Some(table_for(self.hash_megabytes, parsed));
                }
            }
            "move overhead" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|_| "Move Overhead must be an integer".to_owned())?;
                if parsed > 5_000 {
                    return Err("Move Overhead must be between 0 and 5000 ms".to_owned());
                }
                self.move_overhead_ms = parsed;
            }
            #[cfg(feature = "nnue")]
            "evalfile" => {
                self.evaluator = if value.is_empty() || value == "<empty>" {
                    sanj::Evaluator::classical()
                } else {
                    let bytes = fs::read(value)
                        .map_err(|error| format!("cannot read EvalFile '{value}': {error}"))?;
                    let network = sanj::nnue::Network::from_bytes(&bytes)
                        .map_err(|error| format!("invalid EvalFile '{value}': {error}"))?;
                    sanj::Evaluator::nnue(network)
                };
                if let Some(table) = &mut self.table {
                    table.clear();
                }
            }
            _ => return Err(format!("unknown option '{name}'")),
        }
        Ok(())
    }

    fn start_search(&mut self, parameters: GoParameters) {
        let started = Instant::now();
        let position = self.position.clone();
        let color = position.side_to_move();
        let limits = normalize_limits(&parameters, color, self.move_overhead_ms);
        let game_hashes = self.game_hashes.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let threads = self.threads;
        let evaluator = self.evaluator.clone();
        let table = self
            .table
            .take()
            .unwrap_or_else(|| table_for(self.hash_megabytes, threads));
        let handle = thread::spawn(move || {
            let (result, table) = if threads == 1 {
                let mut position = position;
                let mut searcher =
                    Searcher::with_table_and_evaluator(&worker_stop, table, evaluator);
                let result = searcher.search_started(
                    &mut position,
                    &limits,
                    &game_hashes,
                    started,
                    |info| {
                        let _ = send_line(&format_info(info));
                    },
                );
                (result, searcher.into_table())
            } else {
                search_parallel_with_evaluator(
                    &position,
                    &limits,
                    &game_hashes,
                    &worker_stop,
                    table,
                    ParallelOptions::new(threads, evaluator, started),
                    |info| {
                        let _ = send_line(&format_info(info));
                    },
                )
            };
            let bestmove = result
                .best_move
                .map_or_else(|| "0000".to_owned(), |mv| mv.to_string());
            let _ = send_line(&format!("bestmove {bestmove}"));
            table
        });
        self.active = Some(ActiveSearch { stop, handle });
    }

    fn stop_search(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };
        active.stop.store(true, Ordering::Relaxed);
        self.finish_active(active);
    }

    fn reap_finished(&mut self) {
        let finished = self
            .active
            .as_ref()
            .is_some_and(|active| active.handle.is_finished());
        if finished && let Some(active) = self.active.take() {
            self.finish_active(active);
        }
    }

    fn finish_active(&mut self, active: ActiveSearch) {
        match active.handle.join() {
            Ok(table) => self.table = Some(table),
            Err(_) => {
                eprintln!("NEYRANG search thread terminated unexpectedly");
                self.table = Some(table_for(self.hash_megabytes, self.threads));
            }
        }
    }
}

fn table_for(megabytes: usize, threads: usize) -> TranspositionTable {
    if threads == 1 {
        TranspositionTable::new(megabytes)
    } else {
        TranspositionTable::new_shared(megabytes)
    }
}

struct ActiveSearch {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<TranspositionTable>,
}

fn normalize_limits(parameters: &GoParameters, color: Color, overhead_ms: u64) -> SearchLimits {
    let mut limits = SearchLimits {
        depth: parameters.depth,
        nodes: parameters.nodes,
        infinite: parameters.infinite || parameters.ponder,
        ..SearchLimits::default()
    };
    if limits.infinite {
        return limits;
    }
    let overhead = Duration::from_millis(overhead_ms);
    if let Some(milliseconds) = parameters.move_time_ms {
        let hard = Duration::from_millis(milliseconds).saturating_sub(overhead);
        limits.soft_time = Some(hard * 9 / 10);
        limits.hard_time = Some(hard);
        return limits;
    }

    let (time_ms, increment_ms) = match color {
        Color::White => (parameters.wtime_ms, parameters.winc_ms),
        Color::Black => (parameters.btime_ms, parameters.binc_ms),
    };
    if let Some(time_ms) = time_ms {
        let budget = TimeManager::allocate(
            Duration::from_millis(time_ms),
            Duration::from_millis(increment_ms.unwrap_or(0)),
            parameters.moves_to_go,
            overhead,
        );
        limits.soft_time = Some(budget.soft);
        limits.hard_time = Some(budget.hard);
    } else if limits.depth.is_none() && limits.nodes.is_none() {
        limits.infinite = true;
    }
    limits
}

fn format_info(info: &SearchInfo) -> String {
    let score = if info.score.abs() >= VALUE_MATE - MAX_PLY as i32 {
        let plies = VALUE_MATE - info.score.abs();
        let moves = (plies + 1) / 2;
        format!("mate {}", if info.score >= 0 { moves } else { -moves })
    } else {
        format!("cp {}", info.score)
    };
    let pv = info
        .pv
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "info depth {} seldepth {} score {} nodes {} nps {} hashfull {} time {} pv {}",
        info.depth,
        info.seldepth,
        score,
        info.nodes,
        info.nps(),
        info.hashfull,
        info.elapsed.as_millis(),
        pv
    )
}

fn send_line(line: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "{line}")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use crate::chess::Color;

    use super::{DEFAULT_MOVE_OVERHEAD_MS, GoParameters, UciEngine, normalize_limits};

    #[test]
    fn default_overhead_covers_observed_external_latency_tail() {
        let engine = UciEngine::new();

        assert_eq!(engine.move_overhead_ms, DEFAULT_MOVE_OVERHEAD_MS);
        assert_eq!(DEFAULT_MOVE_OVERHEAD_MS, 30);
    }

    #[test]
    fn movetime_stops_immediately_when_overhead_consumes_the_request() {
        let parameters = GoParameters {
            move_time_ms: Some(5),
            ..GoParameters::default()
        };

        let limits = normalize_limits(&parameters, Color::White, 10);

        assert_eq!(limits.soft_time, Some(std::time::Duration::ZERO));
        assert_eq!(limits.hard_time, Some(std::time::Duration::ZERO));
    }

    #[test]
    fn threads_option_switches_between_local_and_total_hash_shared_tables() {
        let mut engine = UciEngine::new();
        assert!(!engine.table.as_ref().expect("default TT").is_shared());

        engine
            .set_option("Threads", Some("4"))
            .expect("Threads 4 should be accepted");
        let shared = engine.table.as_ref().expect("shared TT");
        assert!(shared.is_shared());
        assert!(shared.allocated_bytes() <= engine.hash_megabytes * 1024 * 1024);

        engine
            .set_option("Threads", Some("2"))
            .expect("Threads 2 should retain shared mode");
        assert!(engine.table.as_ref().expect("shared TT").is_shared());

        engine
            .set_option("Threads", Some("1"))
            .expect("Threads 1 should be accepted");
        assert!(!engine.table.as_ref().expect("local TT").is_shared());
    }
}
