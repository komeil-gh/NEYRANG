use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    sync::Arc,
};

use bullet::game::formats::bulletformat::ChessBoard;
use bullet_trainer::reader::DataReader;
use neyrang::chess::Position;

use crate::{POSITION_SHUFFLE_SEED, deterministic_shuffle};

const HEADER: &str = "# neyrang-teacher-wdl-logit400-v1";

#[derive(Clone, Debug)]
/// Bounded, pre-parsed teacher input. Corpus provenance and quiet-move/result
/// eligibility must be audited before export; a text row cannot prove them.
pub struct TeacherTextLoader {
    positions: Arc<[ChessBoard]>,
}

impl TeacherTextLoader {
    pub fn new(path: impl AsRef<Path>, expected: usize, limit: usize) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("open teacher input: {e}"))?;
        Self::from_reader(BufReader::new(file), expected, limit)
    }

    fn from_reader(
        mut reader: impl BufRead,
        expected: usize,
        limit: usize,
    ) -> Result<Self, String> {
        if expected == 0 || expected > limit {
            return Err("teacher row count must be positive and fit the loader buffer".into());
        }
        let mut positions = Vec::with_capacity(expected);
        let mut line = String::new();
        let mut line_number = 0;
        loop {
            line.clear();
            if (&mut reader)
                .take(257)
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                break;
            }
            line_number += 1;
            if line.len() > 256 {
                return Err(format!("teacher line {line_number} exceeds 256 bytes"));
            }
            if line_number == 1 {
                if line.trim_end() != HEADER {
                    return Err("missing teacher target encoding header".into());
                }
                continue;
            }
            if positions.len() == expected {
                return Err("teacher input contains more than the registered row count".into());
            }
            positions
                .push(parse_row(&line).map_err(|e| format!("teacher line {line_number}: {e}"))?);
        }
        if positions.len() != expected {
            return Err(format!(
                "teacher input has {} rows, expected {expected}",
                positions.len()
            ));
        }
        // ponytail: whole bounded corpus in memory; use audited streaming shards if it outgrows the buffer.
        deterministic_shuffle(&mut positions, POSITION_SHUFFLE_SEED);
        Ok(Self {
            positions: positions.into(),
        })
    }
}

impl DataReader<ChessBoard> for TeacherTextLoader {
    fn read_chunks<F: FnMut(&[ChessBoard]) -> bool>(&self, skip_count: usize, mut f: F) {
        let mut offset = skip_count % self.positions.len();
        loop {
            if f(&self.positions[offset..]) {
                return;
            }
            offset = 0;
        }
    }
}

fn parse_row(line: &str) -> Result<ChessBoard, String> {
    let fields: Vec<_> = line.split('|').map(str::trim).collect();
    let [fen, score, result] = fields.as_slice() else {
        return Err("expected FEN | White logit400 score | actual White result".into());
    };
    let score: i16 = score.parse().map_err(|_| "invalid integer teacher score")?;
    if !(-3040..=3040).contains(&score) || !matches!(*result, "0.0" | "0.5" | "1.0") {
        return Err("teacher score or actual result outside encoding contract".into());
    }
    // The upstream text parser assumes valid ranks and kings. Reuse our checked
    // FEN parser before calling it; history and move eligibility belong to export.
    let position = Position::from_fen(fen).map_err(|e| e.to_string())?;
    if position.all_occupancy().count_ones() > 32 {
        return Err("Bullet input supports at most 32 pieces".into());
    }
    line.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bullet::value::loader::LoadableDataType;

    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    fn corpus(rows: &[String]) -> String {
        format!("{HEADER}\n{}\n", rows.join("\n"))
    }

    fn scores(loader: &TeacherTextLoader, skip: usize, count: usize) -> Vec<i16> {
        let mut out = Vec::new();
        loader.read_chunks(skip, |rows| {
            out.extend(rows.iter().take(count - out.len()).map(|r| r.score()));
            out.len() == count
        });
        out
    }

    #[test]
    fn preserves_white_score_and_real_result_orientation() {
        for (turn, sign) in [("w", 1), ("b", -1)] {
            for (result, expected) in [("0.0", 0), ("0.5", 1), ("1.0", 2)] {
                let fen = FEN.replace(" w ", &format!(" {turn} "));
                let text = corpus(&[format!("{fen} | 555 | {result}")]);
                let loader = TeacherTextLoader::from_reader(text.as_bytes(), 1, 1).unwrap();
                assert_eq!(loader.positions[0].score(), sign * 555);
                assert_eq!(
                    loader.positions[0].result() as u8,
                    if turn == "w" { expected } else { 2 - expected }
                );
            }
        }
    }

    #[test]
    fn deterministic_order_repeats_and_skip_is_in_emitted_order() {
        let text = corpus(
            &(0..8)
                .map(|n| format!("{FEN} | {n} | 0.5"))
                .collect::<Vec<_>>(),
        );
        let a = TeacherTextLoader::from_reader(text.as_bytes(), 8, 8).unwrap();
        let b = TeacherTextLoader::from_reader(text.as_bytes(), 8, 8).unwrap();
        let order = scores(&a, 0, 8);
        assert_eq!(order, scores(&b, 0, 8));
        assert_ne!(order, (0..8).collect::<Vec<_>>());
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..8).collect::<Vec<_>>());
        assert_eq!(scores(&a, 0, 16), order.repeat(2));
        for skip in [0, 1, 7, 8, 9, 25] {
            assert_eq!(
                scores(&a, skip, 12),
                order
                    .iter()
                    .cycle()
                    .skip(skip)
                    .take(12)
                    .copied()
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn rejects_bad_contract_counts_and_unsafe_parser_inputs() {
        let valid = corpus(&[format!("{FEN} | 0 | 0.5")]);
        for (expected, limit) in [(0, 1), (1, 0), (2, 1), (2, 2)] {
            assert!(TeacherTextLoader::from_reader(valid.as_bytes(), expected, limit).is_err());
        }
        for text in [
            String::new(),
            valid.replace(HEADER, "# cp"),
            format!("{valid}{FEN} | 0 | 0.5\n"),
        ] {
            assert!(TeacherTextLoader::from_reader(text.as_bytes(), 1, 1).is_err());
        }
        for row in [
            format!("{FEN} | -32768 | 0.5"),
            format!("{FEN} | 3041 | 0.5"),
            format!("{FEN} | 0 | *"),
            format!("{FEN} | 0 | NaN"),
            format!("{FEN} | 0 | 0.5 | extra"),
            format!("{FEN} | 0.5 | 0.5"),
            "8/8/8/8/8/8/8/8 b - - 0 1 | 0 | 0.5".into(),
            "8/8/8/8/8/8/8/8/8 b - - 0 1 | 0 | 0.5".into(),
            format!("{} | 0 | 0.5", FEN.replace(" w ", " x ")),
            format!("{} | 0 | 0.5", FEN.replace("pppppppp", "ppppppppp")),
            "x".repeat(257),
            String::new(),
        ] {
            let text = corpus(&[row]);
            assert!(
                TeacherTextLoader::from_reader(text.as_bytes(), 1, 1).is_err(),
                "{text}"
            );
        }
    }
}
