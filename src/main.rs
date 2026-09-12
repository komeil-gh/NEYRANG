/*
    NEYRANG, a UCI chess engine

    Copyright (C) 2026 Komeil Ghasemi

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed WITHOUT ANY WARRANTY; without even the
    implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

    See the GNU General Public License for more details.
    You should have received a copy of the GNU General Public License
    along with this program. If not, see <https://www.gnu.org/licenses/>.
*/

use std::{env, process::ExitCode, time::Instant};

#[cfg(feature = "nnue")]
use std::fs;
#[cfg(feature = "sanj-tools")]
use std::{
    fs::File,
    io::{self, BufReader, BufWriter},
};

#[cfg(feature = "nnue")]
use neyrang::sanj::{Evaluator, nnue::Network};
#[cfg(feature = "sanj-tools")]
use neyrang::tools::sanj_trace;
use neyrang::{
    chess::{Position, divide, perft},
    engine::info::{ENGINE_NAME, ENGINE_VERSION},
    tools::{bench, genfens},
    uci,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{ENGINE_NAME}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if let Some(command_line) = arguments.first().filter(|argument| {
        argument
            .split_ascii_whitespace()
            .next()
            .is_some_and(|command| command == "genfens")
            && argument.contains(char::is_whitespace)
    }) {
        if arguments.len() != 2 || arguments[1] != "quit" {
            return Err(
                "quoted OpenBench genfens invocation must be followed by exactly 'quit'".to_owned(),
            );
        }
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        genfens::run(command_line, &mut output).map_err(|error| error.to_string())?;
        return Ok(());
    }
    let Some(command) = arguments.first().map(String::as_str) else {
        return uci::run().map_err(|error| error.to_string());
    };
    match command {
        "perft" => {
            let depth = parse_depth(arguments.get(1))?;
            let mut position = Position::startpos();
            println!("{}", perft(&mut position, depth));
        }
        "divide" => {
            let depth = parse_depth(arguments.get(1))?;
            let mut position = Position::startpos();
            let divisions = divide(&mut position, depth);
            let total: u64 = divisions.iter().map(|(_, nodes)| nodes).sum();
            for (mv, nodes) in divisions {
                println!("{mv}: {nodes}");
            }
            println!("Total: {total}");
        }
        "perft-fen" => {
            let fen = arguments
                .get(1)
                .ok_or_else(|| "perft-fen requires a quoted FEN and depth".to_owned())?;
            let depth = parse_depth(arguments.get(2))?;
            let mut position = Position::from_fen(fen).map_err(|error| error.to_string())?;
            println!("{}", perft(&mut position, depth));
        }
        "bench" => {
            let depth = arguments
                .get(1)
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| "bench depth must be 1..127".to_owned())
                })
                .transpose()?
                .unwrap_or(5_u8);
            if depth == 0 || depth as usize >= neyrang::rekhne::MAX_PLY {
                return Err("bench depth must be 1..127".to_owned());
            }
            let started = Instant::now();
            let result = bench::run(depth)?;
            println!("positions: {}", result.positions);
            println!("nodes: {}", result.nodes);
            println!("time: {} ms", result.elapsed.as_millis());
            println!("nps: {}", result.nps());
            println!("checksum: {:016x}", result.checksum);
            #[cfg(feature = "stats")]
            print_benchmark_statistics(result.statistics);
            let _ = started;
        }
        #[cfg(feature = "nnue")]
        "bench-nnue" => {
            let path = arguments
                .get(1)
                .ok_or_else(|| "bench-nnue requires a network path".to_owned())?;
            if arguments.len() > 3 {
                return Err("bench-nnue accepts a network path and optional depth".to_owned());
            }
            let depth = arguments
                .get(2)
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| "bench-nnue depth must be 1..127".to_owned())
                })
                .transpose()?
                .unwrap_or(5_u8);
            if depth == 0 || depth as usize >= neyrang::rekhne::MAX_PLY {
                return Err("bench-nnue depth must be 1..127".to_owned());
            }
            let bytes = fs::read(path)
                .map_err(|error| format!("cannot read NNUE network '{path}': {error}"))?;
            let network = Network::from_bytes(&bytes)
                .map_err(|error| format!("invalid NNUE network '{path}': {error}"))?;
            let result = bench::run_with_evaluator(depth, Evaluator::nnue(network))?;
            println!("positions: {}", result.positions);
            println!("nodes: {}", result.nodes);
            println!("time: {} ms", result.elapsed.as_millis());
            println!("nps: {}", result.nps());
            println!("checksum: {:016x}", result.checksum);
            #[cfg(feature = "stats")]
            print_benchmark_statistics(result.statistics);
        }
        "genfens" => {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            genfens::run(&arguments.join(" "), &mut output).map_err(|error| error.to_string())?;
        }
        #[cfg(feature = "sanj-tools")]
        "sanj-trace" => {
            let input = arguments
                .get(1)
                .ok_or_else(|| "sanj-trace requires a TSV path or '-' for stdin".to_owned())?;
            if arguments.len() != 2 {
                return Err("sanj-trace accepts exactly one TSV path or '-'".to_owned());
            }

            let stdout = io::stdout();
            let output = BufWriter::new(stdout.lock());
            if input == "-" {
                let stdin = io::stdin();
                sanj_trace::export(stdin.lock(), output)?;
            } else {
                let file = File::open(input)
                    .map_err(|error| format!("cannot open sanj-trace input '{input}': {error}"))?;
                sanj_trace::export(BufReader::new(file), output)?;
            }
        }
        "--version" | "-V" => println!("{ENGINE_NAME} {ENGINE_VERSION}"),
        "--help" | "-h" => print_help(),
        _ => return Err(format!("unknown command '{command}' (try --help)")),
    }
    Ok(())
}

#[cfg(feature = "stats")]
fn print_benchmark_statistics(statistics: neyrang::rekhne::SearchStatistics) {
    println!(
        "stats.move_generation_calls: {}",
        statistics.move_generation_calls
    );
    println!("stats.moves_generated: {}", statistics.moves_generated);
    println!("stats.ordering_calls: {}", statistics.ordering_calls);
    println!("stats.moves_scored: {}", statistics.moves_scored);
    println!("stats.full_sorts: {}", statistics.full_sorts);
    println!("stats.moves_searched: {}", statistics.moves_searched);
    println!(
        "stats.scored_moves_searched: {}",
        statistics.scored_moves_searched
    );
    println!(
        "stats.moves_scored_unused: {}",
        statistics.moves_scored_unused()
    );
    println!("stats.see_calls: {}", statistics.see_calls);
    println!(
        "stats.see_scored_moves_searched: {}",
        statistics.see_scored_moves_searched
    );
    println!(
        "stats.see_scored_moves_unused: {}",
        statistics.see_scored_moves_unused()
    );
    println!("stats.beta_cutoffs: {}", statistics.beta_cutoffs);
    println!(
        "stats.null_move_attempts: {}",
        statistics.null_move_attempts
    );
    println!(
        "stats.null_move_fail_highs: {}",
        statistics.null_move_fail_highs
    );
    println!("stats.null_move_cutoffs: {}", statistics.null_move_cutoffs);
    println!(
        "stats.null_move_verifications: {}",
        statistics.null_move_verifications
    );
    println!("stats.lmr_reductions: {}", statistics.lmr_reductions);
    println!("stats.lmr_researches: {}", statistics.lmr_researches);
    println!(
        "stats.first_move_beta_cutoffs: {}",
        statistics.first_move_beta_cutoffs
    );
    println!(
        "stats.picker_tt_stage_visits: {}",
        statistics.picker_tt_stage_visits
    );
    println!(
        "stats.picker_good_tactical_stage_visits: {}",
        statistics.picker_good_tactical_stage_visits
    );
    println!(
        "stats.picker_killer_stage_visits: {}",
        statistics.picker_killer_stage_visits
    );
    println!(
        "stats.picker_quiet_stage_visits: {}",
        statistics.picker_quiet_stage_visits
    );
    println!(
        "stats.picker_bad_tactical_stage_visits: {}",
        statistics.picker_bad_tactical_stage_visits
    );
}

fn parse_depth(value: Option<&String>) -> Result<u8, String> {
    let depth: u8 = value
        .ok_or_else(|| "command requires a depth".to_owned())?
        .parse()
        .map_err(|_| "depth must be an unsigned 8-bit integer".to_owned())?;
    if depth as usize >= neyrang::rekhne::MAX_PLY {
        return Err("depth must be below 128".to_owned());
    }
    Ok(depth)
}

fn print_help() {
    println!("{ENGINE_NAME} {ENGINE_VERSION}");
    println!("Usage:");
    println!("  neyrang                         Run the UCI engine");
    println!("  neyrang perft <depth>           Perft from the initial position");
    println!("  neyrang divide <depth>          Per-move Perft from the initial position");
    println!("  neyrang perft-fen <FEN> <depth> Perft from a FEN");
    println!("  neyrang bench [depth]           Deterministic search benchmark");
    #[cfg(feature = "nnue")]
    println!("  neyrang bench-nnue <FILE> [d]   NNUE search benchmark");
    println!("  neyrang genfens ...             Seeded OpenBench opening generation");
    #[cfg(feature = "sanj-tools")]
    println!("  neyrang sanj-trace <TSV|->      Export exact SANJ features");
}
