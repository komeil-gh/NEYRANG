use std::{env, process::ExitCode, time::Instant};

use neyrang::{
    chess::{Position, divide, perft},
    engine::info::{ENGINE_NAME, ENGINE_VERSION},
    tools::bench,
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
            if depth == 0 || depth as usize >= neyrang::search::MAX_PLY {
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
        "--version" | "-V" => println!("{ENGINE_NAME} {ENGINE_VERSION}"),
        "--help" | "-h" => print_help(),
        _ => return Err(format!("unknown command '{command}' (try --help)")),
    }
    Ok(())
}

#[cfg(feature = "stats")]
fn print_benchmark_statistics(statistics: neyrang::search::SearchStatistics) {
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
        "stats.first_move_beta_cutoffs: {}",
        statistics.first_move_beta_cutoffs
    );
    println!(
        "stats.beta_cutoff_searched_moves: {}",
        statistics.beta_cutoff_searched_moves
    );
    println!(
        "stats.capture_beta_cutoffs: {}",
        statistics.capture_beta_cutoffs
    );
    println!(
        "stats.capture_history_probes: {}",
        statistics.capture_history_probes
    );
    println!(
        "stats.capture_history_nonzero_probes: {}",
        statistics.capture_history_nonzero_probes
    );
    println!(
        "stats.capture_history_reorderings: {}",
        statistics.capture_history_reorderings
    );
    println!(
        "stats.capture_history_reward_updates: {}",
        statistics.capture_history_reward_updates
    );
    println!(
        "stats.capture_history_malus_updates: {}",
        statistics.capture_history_malus_updates
    );
    println!(
        "stats.capture_history_distribution: {:?}",
        statistics.capture_history_distribution
    );
    println!(
        "stats.capture_history_saturated: {}",
        statistics.capture_history_saturated
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
    if depth as usize >= neyrang::search::MAX_PLY {
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
}
