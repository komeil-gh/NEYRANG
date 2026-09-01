use std::{
    collections::HashSet,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use neyrang::chess::Position;
use neyrang_nnue_data::{
    encode_games,
    selfplay::{GameAttempt, SelfPlayConfig, SelfPlaySummary, record_game},
};

const MAX_OPENINGS: usize = 1_000_000;
const MAX_GAME_PLIES: usize = 1_024;
const MAX_HASH_MEGABYTES: usize = 65_536;

struct Options {
    openings: PathBuf,
    output: PathBuf,
    summary: PathBuf,
    config: SelfPlayConfig,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("generate-selfplay: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = parse_options(env::args().skip(1))?;
    refuse_existing(&options.output)?;
    refuse_existing(&options.summary)?;
    let openings = read_openings(&options.openings)?;

    let mut games = Vec::new();
    let mut summary = SelfPlaySummary::default();
    let stdout = io::stdout();
    let mut progress = stdout.lock();
    for opening in openings {
        let attempt = record_game(opening, options.config);
        summary.record(&attempt);
        if let GameAttempt::Accepted(accepted) = attempt {
            games.push(accepted.game);
        }
        writeln!(
            progress,
            "info string selfplay attempted {} accepted {} rejected {} positions {}",
            summary.attempted_games,
            summary.accepted_games,
            summary.rejected_games,
            summary.accepted_positions
        )
        .map_err(|error| format!("cannot write progress: {error}"))?;
        progress
            .flush()
            .map_err(|error| format!("cannot flush progress: {error}"))?;
    }

    if games.is_empty() {
        return Err(format!(
            "no games were accepted (attempted {}, rejected {})",
            summary.attempted_games, summary.rejected_games
        ));
    }
    let encoded = encode_games(&games).map_err(|error| error.to_string())?;
    publish(&options.output, &encoded, &options.summary, &summary)
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let arguments = arguments.collect::<Vec<_>>();
    if arguments.len() != 12 {
        return Err(
            "usage: generate-selfplay --openings FILE --output FILE --summary FILE \
             --nodes N --hash-mb N --max-plies N"
                .to_owned(),
        );
    }
    let mut openings = None;
    let mut output = None;
    let mut summary = None;
    let mut nodes_per_move = None;
    let mut hash_megabytes = None;
    let mut maximum_plies = None;
    for index in (0..arguments.len()).step_by(2) {
        let option = arguments[index].as_str();
        let value = &arguments[index + 1];
        match option {
            "--openings" if openings.is_none() => openings = Some(PathBuf::from(value)),
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--summary" if summary.is_none() => summary = Some(PathBuf::from(value)),
            "--nodes" if nodes_per_move.is_none() => {
                nodes_per_move = Some(parse_positive(value, "nodes")?)
            }
            "--hash-mb" if hash_megabytes.is_none() => {
                hash_megabytes = Some(parse_positive(value, "hash-mb")?)
            }
            "--max-plies" if maximum_plies.is_none() => {
                maximum_plies = Some(parse_positive(value, "max-plies")?)
            }
            option => return Err(format!("unknown or repeated option {option}")),
        }
    }
    let maximum_plies = maximum_plies.ok_or("missing --max-plies")?;
    if maximum_plies > MAX_GAME_PLIES {
        return Err(format!("max-plies must not exceed {MAX_GAME_PLIES}"));
    }
    let hash_megabytes = hash_megabytes.ok_or("missing --hash-mb")?;
    if hash_megabytes > MAX_HASH_MEGABYTES {
        return Err(format!("hash-mb must not exceed {MAX_HASH_MEGABYTES}"));
    }
    Ok(Options {
        openings: openings.ok_or("missing --openings")?,
        output: output.ok_or("missing --output")?,
        summary: summary.ok_or("missing --summary")?,
        config: SelfPlayConfig {
            nodes_per_move: nodes_per_move.ok_or("missing --nodes")?,
            hash_megabytes,
            maximum_plies,
        },
    })
}

fn parse_positive<T>(value: &str, label: &str) -> Result<T, String>
where
    T: std::str::FromStr + PartialEq + Default,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| format!("{label} must be a positive integer"))?;
    if parsed == T::default() {
        return Err(format!("{label} must be a positive integer"));
    }
    Ok(parsed)
}

fn read_openings(path: &Path) -> Result<Vec<Position>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err("opening shard must be non-empty and LF-terminated".to_owned());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("opening shard is not UTF-8: {error}"))?;
    if text.contains('\r') {
        return Err("opening shard must use LF rather than CRLF".to_owned());
    }
    let mut positions = Vec::new();
    let mut canonical = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            return Err(format!("opening line {} is empty", index + 1));
        }
        let position = Position::from_fen(line)
            .map_err(|error| format!("invalid FEN on opening line {}: {error}", index + 1))?;
        if !canonical.insert(position.to_fen()) {
            return Err(format!("duplicate opening on line {}", index + 1));
        }
        positions.push(position);
        if positions.len() > MAX_OPENINGS {
            return Err(format!("opening shard exceeds {MAX_OPENINGS} positions"));
        }
    }
    Ok(positions)
}

fn refuse_existing(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "parent directory does not exist: {}",
            parent.display()
        ));
    }
    Ok(())
}

fn publish(
    output_path: &Path,
    output_bytes: &[u8],
    summary_path: &Path,
    summary: &SelfPlaySummary,
) -> Result<(), String> {
    write_new(output_path, output_bytes)?;
    let summary_bytes = summary_json(summary).into_bytes();
    if let Err(error) = write_new(summary_path, &summary_bytes) {
        let _ = fs::remove_file(output_path);
        return Err(error);
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::fs::OpenOptions;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            format!(
                "cannot create {} without overwrite: {error}",
                path.display()
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("cannot publish {}: {error}", path.display()));
    }
    Ok(())
}

fn summary_json(summary: &SelfPlaySummary) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"neyrang-selfplay-summary-v1\",\n",
            "  \"attempted_games\": {},\n",
            "  \"accepted_games\": {},\n",
            "  \"rejected_games\": {},\n",
            "  \"accepted_positions\": {},\n",
            "  \"checkmates\": {},\n",
            "  \"stalemates\": {},\n",
            "  \"fifty_move_draws\": {},\n",
            "  \"threefold_draws\": {},\n",
            "  \"rejected_terminal_openings\": {},\n",
            "  \"rejected_maximum_plies\": {},\n",
            "  \"rejected_missing_search_move\": {},\n",
            "  \"rejected_illegal_search_move\": {},\n",
            "  \"rejected_search_mutated_position\": {}\n",
            "}}\n"
        ),
        summary.attempted_games,
        summary.accepted_games,
        summary.rejected_games,
        summary.accepted_positions,
        summary.checkmates,
        summary.stalemates,
        summary.fifty_move_draws,
        summary.threefold_draws,
        summary.rejected_terminal_openings,
        summary.rejected_maximum_plies,
        summary.rejected_missing_search_move,
        summary.rejected_illegal_search_move,
        summary.rejected_search_mutated_position,
    )
}

#[cfg(test)]
mod tests {
    use super::parse_options;

    #[test]
    fn cli_rejects_hash_sizes_above_the_engine_contract() {
        let arguments = [
            "--openings",
            "openings.epd",
            "--output",
            "games.vf",
            "--summary",
            "summary.json",
            "--nodes",
            "1",
            "--hash-mb",
            "65537",
            "--max-plies",
            "1",
        ]
        .into_iter()
        .map(str::to_owned);

        let result = parse_options(arguments);

        assert!(matches!(result, Err(error) if error.contains("65536")));
    }
}
