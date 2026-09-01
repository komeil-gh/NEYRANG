use std::io::{BufRead, Write};

use crate::{
    chess::Position,
    sanj::{TRACE_COLUMNS, TRACE_SCHEMA, trace},
};

pub const EXPORT_HEADER: &str = "schema\trecord_id\ttarget\tfen";

/// Stream `record_id<TAB>target<TAB>FEN` records to the fixed trace schema.
pub fn export<R: BufRead, W: Write>(reader: R, mut writer: W) -> Result<usize, String> {
    writeln!(writer, "{EXPORT_HEADER}\t{TRACE_COLUMNS}")
        .map_err(|error| format!("cannot write sanj-trace header: {error}"))?;

    let mut exported = 0;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|error| format!("sanj-trace line {line_number}: {error}"))?;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        let mut fields = line.split('\t');
        let record_id = required_field(fields.next(), line_number, "record_id")?;
        let target_text = required_field(fields.next(), line_number, "target")?;
        let fen = required_field(fields.next(), line_number, "FEN")?;
        if fields.next().is_some() {
            return Err(format!(
                "sanj-trace line {line_number}: expected exactly three tab-separated fields"
            ));
        }
        if record_id.trim() != record_id || target_text.trim() != target_text || fen.trim() != fen {
            return Err(format!(
                "sanj-trace line {line_number}: fields must not have leading or trailing whitespace"
            ));
        }

        let target: f64 = target_text.parse().map_err(|_| {
            format!("sanj-trace line {line_number}: target must be a number within [0,1]")
        })?;
        if !target.is_finite() || !(0.0..=1.0).contains(&target) {
            return Err(format!(
                "sanj-trace line {line_number}: target must be finite and within [0,1]"
            ));
        }
        let position = Position::from_fen(fen)
            .map_err(|error| format!("sanj-trace line {line_number}: invalid FEN: {error}"))?;
        let canonical_fen = position.to_fen();
        let evaluation = trace(&position);
        writeln!(
            writer,
            "{TRACE_SCHEMA}\t{record_id}\t{target}\t{canonical_fen}\t{evaluation}"
        )
        .map_err(|error| format!("cannot write sanj-trace line {line_number}: {error}"))?;
        exported += 1;
    }
    writer
        .flush()
        .map_err(|error| format!("cannot flush sanj-trace output: {error}"))?;
    Ok(exported)
}

fn required_field<'a>(
    field: Option<&'a str>,
    line_number: usize,
    name: &str,
) -> Result<&'a str, String> {
    match field {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(format!(
            "sanj-trace line {line_number}: missing or empty {name}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn exports_multiple_records_with_canonical_fixed_width_rows() {
        let input = concat!(
            "# record_id<TAB>target<TAB>FEN\n",
            "start\t0.5\trnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\n",
            "\n",
            "queen\t1\t7k/8/8/8/8/8/4Q3/7K w - - 0 1\n",
        );
        let mut output = Vec::new();
        assert_eq!(export(Cursor::new(input), &mut output), Ok(2));
        let output = String::from_utf8(output).expect("trace output is UTF-8");
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], format!("{EXPORT_HEADER}\t{TRACE_COLUMNS}"));
        let width = lines[0].split('\t').count();
        assert_eq!(width, 38);
        assert!(lines[1].starts_with("neyrang-sanj-trace-v1\tstart\t0.5\t"));
        assert!(lines[2].starts_with("neyrang-sanj-trace-v1\tqueen\t1\t"));
        assert!(lines.iter().all(|line| line.split('\t').count() == width));
    }

    #[test]
    fn malformed_records_fail_closed_with_line_numbers() {
        let input = concat!(
            "ok\t0.5\trnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\n",
            "# ignored\n",
            "bad\t1.5\t7k/8/8/8/8/8/4Q3/7K w - - 0 1\n",
        );
        let error = export(Cursor::new(input), Vec::new()).expect_err("invalid target must fail");
        assert!(error.contains("line 3"));
        assert!(error.contains("within [0,1]"));
    }

    #[test]
    fn extra_fields_are_rejected() {
        let input = "bad\t0.5\t7k/8/8/8/8/8/4Q3/7K w - - 0 1\textra\n";
        let error = export(Cursor::new(input), Vec::new()).expect_err("extra field must fail");
        assert!(error.contains("exactly three"));
    }
}
