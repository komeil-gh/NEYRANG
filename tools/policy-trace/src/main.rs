use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    process::ExitCode,
    str::FromStr,
};

use neyrang::{
    chess::{Color, PieceType, Position, Square},
    shegerd::see,
};

const TRACE_SCHEMA: &str = "neyrang-shegerd-policy-trace-v1";
const HEADER: &str = "schema\trecord_id\tgroup_id\tcandidate_move\tselected\tstage\tfrom_normalized\tto_normalized\tmover\tvictim\tpromotion\tphase\tprevious_to_normalized\tsee_bucket\tfen";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("neyrang-policy-trace: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let [input] = arguments.as_slice() else {
        return Err("expected exactly one TSV path or '-' for stdin".to_owned());
    };
    let stdout = io::stdout();
    let output = BufWriter::new(stdout.lock());
    if input == "-" {
        export(io::stdin().lock(), output)
    } else {
        let file = File::open(input)
            .map_err(|error| format!("cannot open policy trace input '{input}': {error}"))?;
        export(BufReader::new(file), output)
    }
}

fn export<R: BufRead, W: Write>(input: R, mut output: W) -> Result<(), String> {
    writeln!(output, "{HEADER}")
        .map_err(|error| format!("cannot write policy trace header: {error}"))?;
    let mut record_ids = HashSet::new();

    for (index, line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|error| format!("line {line_number}: {error}"))?;
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 5 {
            return Err(format!(
                "line {line_number}: expected exactly five tab-separated fields"
            ));
        }
        if fields
            .iter()
            .any(|field| field.is_empty() || field.trim() != *field)
        {
            return Err(format!(
                "line {line_number}: fields must be non-empty without surrounding whitespace"
            ));
        }
        let [record_id, group_id, previous_to, teacher_move, fen] =
            <[&str; 5]>::try_from(fields.as_slice()).expect("field count was checked");
        if !record_ids.insert(record_id.to_owned()) {
            return Err(format!(
                "line {line_number}: duplicate record id '{record_id}'"
            ));
        }

        let mut position = Position::from_fen(fen)
            .map_err(|error| format!("line {line_number}: invalid FEN: {error}"))?;
        let previous_to =
            if previous_to == "-" {
                None
            } else {
                Some(Square::from_str(previous_to).map_err(|error| {
                    format!("line {line_number}: invalid previous square: {error}")
                })?)
            };
        let selected = position
            .find_legal_move(teacher_move)
            .ok_or_else(|| format!("line {line_number}: teacher move is not legal"))?;
        let color = position.side_to_move();
        let previous_to = previous_to.map_or(-1, |square| normalized(square, color) as i32);
        let phase = phase(&position);
        let moves = position.legal_moves();

        for &candidate in moves.iter() {
            let mover = position
                .piece_at(candidate.from())
                .map(|(_, piece)| piece_code(piece))
                .ok_or_else(|| format!("line {line_number}: legal move has no mover"))?;
            let victim = if candidate.is_en_passant() {
                piece_code(PieceType::Pawn)
            } else {
                position
                    .piece_at(candidate.to())
                    .map_or(0, |(_, piece)| piece_code(piece))
            };
            let promotion = candidate.promotion().map_or(0, piece_code);
            let tactical = candidate.is_capture() || candidate.is_promotion();
            let exchange = if tactical {
                see_bucket(see(&position, candidate))
            } else {
                0
            };
            writeln!(
                output,
                "{TRACE_SCHEMA}\t{record_id}\t{group_id}\t{candidate}\t{}\t{}\t{}\t{}\t{mover}\t{victim}\t{promotion}\t{phase}\t{previous_to}\t{exchange}\t{fen}",
                u8::from(candidate == selected),
                if tactical { "tactical" } else { "quiet" },
                normalized(candidate.from(), color),
                normalized(candidate.to(), color),
            )
            .map_err(|error| format!("cannot write row for line {line_number}: {error}"))?;
        }
    }
    output
        .flush()
        .map_err(|error| format!("cannot flush policy trace: {error}"))
}

const fn normalized(square: Square, color: Color) -> usize {
    match color {
        Color::White => square.index(),
        Color::Black => square.index() ^ 56,
    }
}

const fn piece_code(piece: PieceType) -> usize {
    piece.index() + 1
}

fn phase(position: &Position) -> u8 {
    let non_pawn = [
        (PieceType::Knight, 3),
        (PieceType::Bishop, 3),
        (PieceType::Rook, 5),
        (PieceType::Queen, 9),
    ]
    .into_iter()
    .map(|(piece, value)| {
        value
            * (position.pieces(Color::White, piece).count_ones()
                + position.pieces(Color::Black, piece).count_ones())
    })
    .sum::<u32>();
    if non_pawn >= 52 {
        0
    } else if non_pawn >= 24 {
        1
    } else {
        2
    }
}

const fn see_bucket(value: i32) -> i8 {
    if value <= -100 {
        -2
    } else if value < 0 {
        -1
    } else if value == 0 {
        0
    } else if value < 100 {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::{TRACE_SCHEMA, export};

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn output_is_deterministic_complete_and_side_normalized() {
        let input = format!(
            "white\tg1\t-\te2e4\t{START}\nblack\tg1\te4\te7e5\trnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1\n"
        );
        let mut first = Vec::new();
        let mut second = Vec::new();
        export(input.as_bytes(), &mut first).unwrap();
        export(input.as_bytes(), &mut second).unwrap();
        assert_eq!(first, second);

        let text = String::from_utf8(first).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 41);
        assert!(lines.iter().all(|line| line.split('\t').count() == 15));
        assert_eq!(
            lines[1..21]
                .iter()
                .filter(|line| line.contains("\t1\tquiet\t"))
                .count(),
            1
        );
        assert_eq!(
            lines[21..]
                .iter()
                .filter(|line| line.contains("\t1\tquiet\t"))
                .count(),
            1
        );
        assert!(text.contains(&format!(
            "{TRACE_SCHEMA}\twhite\tg1\te2e4\t1\tquiet\t12\t28\t1\t0\t0\t0\t-1\t0\t{START}"
        )));
        assert!(text.contains("\tblack\tg1\te7e5\t1\tquiet\t12\t28\t1\t0\t0\t0\t36\t0\t"));
    }

    #[test]
    fn output_contains_native_tactical_features() {
        let fen = "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1";
        let input = format!("capture\tg2\td5\tc4d5\t{fen}\n");
        let mut output = Vec::new();
        export(input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\tc4d5\t1\ttactical\t26\t35\t1\t5\t0\t2\t35\t2\t"));
        assert!(text.contains("\te4e5\t0\ttactical\t28\t36\t5\t1\t0\t2\t35\t-2\t"));
    }

    #[test]
    fn output_distinguishes_en_passant_and_quiet_promotion() {
        let input = "ep\tg3\td5\te5d6\t4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1\npromo\tg4\t-\ta7a8q\t4k3/P7/8/8/8/8/8/4K3 w - - 0 1\n";
        let mut output = Vec::new();
        export(input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\tep\tg3\te5d6\t1\ttactical\t36\t43\t1\t1\t0\t2\t35\t2\t"));
        assert!(text.contains("\tpromo\tg4\ta7a8q\t1\ttactical\t48\t56\t1\t0\t5\t2\t-1\t2\t"));
    }

    #[test]
    fn malformed_or_ambiguous_input_is_rejected() {
        for input in [
            format!("id\tg\t-\te2e5\t{START}\n"),
            format!("id\tg\tz9\te2e4\t{START}\n"),
            format!("id\tg\t-\te2e4\t{START}\nid\tg\t-\te2e4\t{START}\n"),
            format!(" id\tg\t-\te2e4\t{START}\n"),
            "id\tg\t-\te2e4\n".to_owned(),
        ] {
            assert!(export(input.as_bytes(), Vec::new()).is_err());
        }
    }
}
