use std::{collections::HashSet, fmt};

use neyrang::chess::Position;

/// One immutable FEN and its parsed engine position.
#[derive(Clone, Debug)]
pub struct FrozenPosition {
    pub fen: String,
    pub position: Position,
}

/// Fail-closed errors for a line-oriented parity suite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FenSuiteError {
    Empty,
    InvalidFen { line: usize, reason: String },
    DuplicatePosition { line: usize },
}

impl fmt::Display for FenSuiteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("FEN suite contains no positions"),
            Self::InvalidFen { line, reason } => {
                write!(formatter, "invalid FEN on line {line}: {reason}")
            }
            Self::DuplicatePosition { line } => {
                write!(formatter, "duplicate canonical position on line {line}")
            }
        }
    }
}

impl std::error::Error for FenSuiteError {}

/// Parse full six-field FENs, ignoring only blank and whole-line comment records.
pub fn parse_fen_suite(contents: &str) -> Result<Vec<FrozenPosition>, FenSuiteError> {
    let mut positions = Vec::new();
    let mut canonical_positions = HashSet::new();
    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let fen = raw_line.trim();
        if fen.is_empty() || fen.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = fen.split_whitespace().collect();
        if fields.len() != 6 {
            return Err(FenSuiteError::InvalidFen {
                line: line_number,
                reason: format!("expected six fields, found {}", fields.len()),
            });
        }
        let canonical = fields[..4].join(" ");
        if !canonical_positions.insert(canonical) {
            return Err(FenSuiteError::DuplicatePosition { line: line_number });
        }
        let position = Position::from_fen(fen).map_err(|error| FenSuiteError::InvalidFen {
            line: line_number,
            reason: error.to_string(),
        })?;
        positions.push(FrozenPosition {
            fen: fen.to_string(),
            position,
        });
    }
    if positions.is_empty() {
        return Err(FenSuiteError::Empty);
    }
    Ok(positions)
}
