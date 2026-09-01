use std::{
    collections::HashMap,
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use neyrang::chess::Color;
use neyrang_nnue_data::decode_games;

const RANK_SCHEMA: &[u8] = b"neyrang-nnue-fen-sample-v1";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sample-fens: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let output = PathBuf::from(args.next().ok_or("missing output path")?);
    let count = parse_count(args.next().ok_or("missing sample count")?)?;
    let seed = args
        .next()
        .ok_or("missing ranking seed")?
        .into_string()
        .map_err(|_| "ranking seed is not UTF-8".to_string())?;
    let inputs: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if inputs.is_empty() {
        return Err("at least one corpus input is required".to_string());
    }
    if output.exists() {
        return Err(format!("refusing to overwrite: {}", output.display()));
    }

    let mut positions_seen = 0_u64;
    let mut unique = HashMap::<String, String>::new();
    for input in &inputs {
        let bytes =
            fs::read(input).map_err(|error| format!("read corpus {}: {error}", input.display()))?;
        let games = decode_games(&bytes)
            .map_err(|error| format!("decode corpus {}: {error}", input.display()))?;
        for game in games {
            let mut position = game.initial_position;
            for scored_move in game.moves {
                let fen = position.to_fen();
                let canonical = canonical_fen(&fen)?;
                unique
                    .entry(canonical)
                    .and_modify(|current| {
                        if fen < *current {
                            *current = fen.clone();
                        }
                    })
                    .or_insert(fen);
                positions_seen += 1;
                position.make_move(scored_move.mv);
            }
        }
    }

    let per_color = count / 2;
    let mut white = Vec::new();
    let mut black = Vec::new();
    for (canonical, fen) in unique {
        let side_to_move = if canonical
            .split_whitespace()
            .nth(1)
            .expect("canonical FEN has side-to-move field")
            == "w"
        {
            Color::White
        } else {
            Color::Black
        };
        let item = (rank(&seed, &canonical), canonical, fen);
        match side_to_move {
            Color::White => white.push(item),
            Color::Black => black.push(item),
        }
    }
    sort_ranked(&mut white);
    sort_ranked(&mut black);
    if white.len() < per_color || black.len() < per_color {
        return Err(format!(
            "insufficient unique positions: need {per_color} per color, found white={} black={}",
            white.len(),
            black.len()
        ));
    }
    let mut selected = Vec::with_capacity(count);
    selected.extend(white.into_iter().take(per_color));
    selected.extend(black.into_iter().take(per_color));
    sort_ranked(&mut selected);
    let output_bytes = selected
        .iter()
        .map(|(_, _, fen)| fen.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    write_exclusive(&output, output_bytes.as_bytes())?;

    println!(
        concat!(
            "{{\"schema\":\"neyrang-nnue-fen-sample-v1\",",
            "\"positions\":{},\"white\":{},\"black\":{},",
            "\"positions_seen\":{},\"inputs\":{},",
            "\"ranking\":\"FNV-1a-64(schema + NUL + seed + NUL + canonical FEN)\"}}"
        ),
        selected.len(),
        per_color,
        per_color,
        positions_seen,
        inputs.len(),
    );
    Ok(())
}

fn parse_count(value: std::ffi::OsString) -> Result<usize, String> {
    let value = value
        .into_string()
        .map_err(|_| "sample count is not UTF-8".to_string())?;
    let count: usize = value
        .parse()
        .map_err(|_| format!("sample count is not a positive even integer: {value:?}"))?;
    if count == 0 || !count.is_multiple_of(2) {
        return Err("sample count must be positive and even".to_string());
    }
    Ok(count)
}

fn canonical_fen(fen: &str) -> Result<String, String> {
    let fields: Vec<_> = fen.split_whitespace().collect();
    if fields.len() != 6 {
        return Err(format!("engine emitted non-six-field FEN: {fen}"));
    }
    Ok(fields[..4].join(" "))
}

fn rank(seed: &str, canonical_fen: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in RANK_SCHEMA
        .iter()
        .copied()
        .chain([0])
        .chain(seed.bytes())
        .chain([0])
        .chain(canonical_fen.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn sort_ranked(values: &mut [(u64, String, String)]) {
    values.sort_unstable_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("create output {}: {error}", path.display()))?;
    if let Err(error) = output.write_all(bytes).and_then(|()| output.sync_all()) {
        drop(output);
        let _ = fs::remove_file(path);
        return Err(format!("write output {}: {error}", path.display()));
    }
    Ok(())
}
