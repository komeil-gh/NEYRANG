use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Uci,
    IsReady,
    NewGame,
    Position(PositionSpecification),
    Go(GoParameters),
    Stop,
    Quit,
    SetOption { name: String, value: Option<String> },
    Display,
    Evaluate,
    Perft(u8),
    Unknown(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionSpecification {
    /// None denotes startpos; Some contains all six FEN fields.
    pub fen: Option<String>,
    pub moves: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GoParameters {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub move_time_ms: Option<u64>,
    pub wtime_ms: Option<u64>,
    pub btime_ms: Option<u64>,
    pub winc_ms: Option<u64>,
    pub binc_ms: Option<u64>,
    pub moves_to_go: Option<u32>,
    pub infinite: bool,
    pub ponder: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError(String);

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ParseError {}

pub fn parse(line: &str) -> Result<Command, ParseError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let Some(&command) = tokens.first() else {
        return Err(ParseError::new("empty UCI command"));
    };
    match command {
        "uci" => Ok(Command::Uci),
        "isready" => Ok(Command::IsReady),
        "ucinewgame" => Ok(Command::NewGame),
        "position" => parse_position(&tokens[1..]).map(Command::Position),
        "go" => parse_go(&tokens[1..]).map(Command::Go),
        "stop" => Ok(Command::Stop),
        "quit" => Ok(Command::Quit),
        "setoption" => parse_setoption(&tokens[1..]),
        "d" => Ok(Command::Display),
        "eval" => Ok(Command::Evaluate),
        "perft" => {
            let depth = tokens
                .get(1)
                .ok_or_else(|| ParseError::new("perft requires a depth"))?
                .parse()
                .map_err(|_| ParseError::new("perft depth must be an unsigned 8-bit integer"))?;
            Ok(Command::Perft(depth))
        }
        unknown => Ok(Command::Unknown(unknown.to_owned())),
    }
}

fn parse_position(tokens: &[&str]) -> Result<PositionSpecification, ParseError> {
    let Some(&kind) = tokens.first() else {
        return Err(ParseError::new("position requires startpos or fen"));
    };
    let moves_index = tokens.iter().position(|&token| token == "moves");
    let base_end = moves_index.unwrap_or(tokens.len());
    let fen = match kind {
        "startpos" => {
            if base_end != 1 {
                return Err(ParseError::new("unexpected token after position startpos"));
            }
            None
        }
        "fen" => {
            if base_end != 7 {
                return Err(ParseError::new(
                    "position fen requires exactly six FEN fields",
                ));
            }
            Some(tokens[1..base_end].join(" "))
        }
        _ => return Err(ParseError::new("position requires startpos or fen")),
    };
    let moves = moves_index
        .map(|index| {
            tokens[index + 1..]
                .iter()
                .map(|token| (*token).to_owned())
                .collect()
        })
        .unwrap_or_default();
    Ok(PositionSpecification { fen, moves })
}

fn parse_go(tokens: &[&str]) -> Result<GoParameters, ParseError> {
    let mut parameters = GoParameters::default();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        match token {
            "infinite" => parameters.infinite = true,
            "ponder" => parameters.ponder = true,
            "depth" => parameters.depth = Some(parse_value(tokens, &mut index, token)?),
            "nodes" => parameters.nodes = Some(parse_value(tokens, &mut index, token)?),
            "movetime" => parameters.move_time_ms = Some(parse_value(tokens, &mut index, token)?),
            "wtime" => parameters.wtime_ms = Some(parse_value(tokens, &mut index, token)?),
            "btime" => parameters.btime_ms = Some(parse_value(tokens, &mut index, token)?),
            "winc" => parameters.winc_ms = Some(parse_value(tokens, &mut index, token)?),
            "binc" => parameters.binc_ms = Some(parse_value(tokens, &mut index, token)?),
            "movestogo" => parameters.moves_to_go = Some(parse_value(tokens, &mut index, token)?),
            unsupported => {
                return Err(ParseError::new(format!(
                    "unsupported go token '{unsupported}'"
                )));
            }
        }
        index += 1;
    }
    Ok(parameters)
}

fn parse_value<T>(tokens: &[&str], index: &mut usize, name: &str) -> Result<T, ParseError>
where
    T: std::str::FromStr,
{
    *index += 1;
    tokens
        .get(*index)
        .ok_or_else(|| ParseError::new(format!("go {name} requires a value")))?
        .parse()
        .map_err(|_| ParseError::new(format!("go {name} has an invalid numeric value")))
}

fn parse_setoption(tokens: &[&str]) -> Result<Command, ParseError> {
    if tokens.first().copied() != Some("name") {
        return Err(ParseError::new("setoption requires 'name'"));
    }
    let value_index = tokens.iter().position(|&token| token == "value");
    let name_end = value_index.unwrap_or(tokens.len());
    if name_end <= 1 {
        return Err(ParseError::new("setoption name cannot be empty"));
    }
    let name = tokens[1..name_end].join(" ");
    let value = value_index.map(|index| tokens[index + 1..].join(" "));
    Ok(Command::SetOption { name, value })
}
