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
            let _ = started;
        }
        "--version" | "-V" => println!("{ENGINE_NAME} {ENGINE_VERSION}"),
        "--help" | "-h" => print_help(),
        _ => return Err(format!("unknown command '{command}' (try --help)")),
    }
    Ok(())
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
