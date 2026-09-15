use std::{env, path::Path, process::ExitCode};

use bullet::{
    game::{
        formats::bulletformat::ChessBoard,
        inputs::{Chess768, ChessBucketsMirrored, get_num_buckets},
        outputs::MaterialCount,
    },
    nn::{
        InitSettings, Shape,
        optimiser::{AdamW, AdamWParams},
    },
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::ValueTrainerBuilder,
};
use bullet_trainer::reader::DataReader;
use neyrang_nnue_trainer::{
    ACTIVATION_QUANT, DeterministicViriLoader, EpochPlan, OUTPUT_BIAS_QUANT, OUTPUT_QUANT,
    POSITION_SHUFFLE_SEED, PositionFilter, TeacherTextLoader,
};

const HIDDEN_SIZE: usize = 128;
const EVAL_SCALE: i32 = 400;
const INITIAL_LR: f32 = 0.001;
const NETWORK_SEED: u64 = 20_260_901;
#[rustfmt::skip]
const KING_BUCKET_LAYOUT_MIRRORED_3: [usize; 32] = [
    1, 1, 1, 0,
    1, 1, 1, 1,
    2, 2, 2, 2,
    2, 2, 2, 2,
    2, 2, 2, 2,
    2, 2, 2, 2,
    2, 2, 2, 2,
    2, 2, 2, 2,
];
const KING_BUCKET_COUNT: usize = get_num_buckets(&KING_BUCKET_LAYOUT_MIRRORED_3);
const PHASE_HEAD_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrainerFeatureSet {
    Chess768,
    Chess768KingBucketsMirrored3,
    Chess768KingBucketsMirrored3PhaseHeads4,
}

impl TrainerFeatureSet {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Chess768 => "chess768",
            Self::Chess768KingBucketsMirrored3 => "chess768x3hm",
            Self::Chess768KingBucketsMirrored3PhaseHeads4 => "chess768x3hm4ph",
        }
    }
}

#[derive(Debug)]
struct Args {
    train: String,
    output_directory: String,
    net_id: String,
    positions: usize,
    batch_size: usize,
    buffer_megabytes: usize,
    threads: usize,
    position_filter: PositionFilter,
    wdl_proportion: f32,
    feature_set: TrainerFeatureSet,
    input_format: String,
}

fn main() -> ExitCode {
    match parse_args().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("neyrang-nnue-trainer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), String> {
    let plan =
        EpochPlan::new(args.positions, args.batch_size).map_err(|error| error.to_string())?;
    if !Path::new(&args.train).is_file() {
        return Err(format!("training corpus is not a file: {}", args.train));
    }
    if Path::new(&args.output_directory).exists() {
        return Err(format!(
            "refusing to reuse output directory: {}",
            args.output_directory
        ));
    }
    if args.buffer_megabytes == 0 || args.threads == 0 {
        return Err("buffer size and loader thread count must be positive".to_string());
    }

    let chunk_positions = args
        .buffer_megabytes
        .checked_mul(1024 * 1024)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<ChessBoard>()))
        .ok_or("loader buffer size overflow")?;
    if args.input_format == "teacher-text-v1" {
        let loader = TeacherTextLoader::new(&args.train, plan.positions, chunk_positions)?;
        train(args, plan, loader)
    } else {
        let loader = DeterministicViriLoader::new(&args.train, chunk_positions)
            .map_err(|error| error.to_string())?
            .with_filter(args.position_filter);
        train(args, plan, loader)
    }
}

fn train(args: Args, plan: EpochPlan, loader: impl DataReader<ChessBoard>) -> Result<(), String> {
    let schedule = TrainingSchedule {
        net_id: args.net_id.clone(),
        eval_scale: EVAL_SCALE as f32,
        steps: TrainingSteps {
            batch_size: plan.batch_size,
            batches_per_superbatch: plan.batches,
            start_superbatch: 1,
            end_superbatch: 1,
        },
        wdl_scheduler: wdl::ConstantWDL {
            value: args.wdl_proportion,
        },
        lr_scheduler: lr::ConstantLR { value: INITIAL_LR },
        save_rate: 1,
    };
    let settings = LocalSettings {
        threads: args.threads,
        test_set: None,
        output_directory: &args.output_directory,
        batch_queue_size: 32,
    };
    println!(
        "NEYRANG epoch contract: positions={} batch_size={} batches={} position_filter={} wdl_proportion={} feature_set={} network_seed={} position_shuffle_seed={} activation_quant={} output_quant={} input_format={} bullet_rev=629ee50000b2afb7b3337595401c830d3b1e0f42",
        plan.positions,
        plan.batch_size,
        plan.batches,
        args.position_filter.as_str(),
        args.wdl_proportion,
        args.feature_set.as_str(),
        NETWORK_SEED,
        POSITION_SHUFFLE_SEED,
        ACTIVATION_QUANT,
        OUTPUT_QUANT,
        args.input_format,
    );
    match args.feature_set {
        TrainerFeatureSet::Chess768 => {
            let mut trainer = ValueTrainerBuilder::default()
                .seed(NETWORK_SEED)
                .dual_perspective()
                .optimiser(AdamW)
                .inputs(Chess768)
                .save_format(&[
                    SavedFormat::id("l0w")
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l0b")
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l1w").round().quantise::<i16>(OUTPUT_QUANT),
                    SavedFormat::id("l1b")
                        .round()
                        .quantise::<i32>(OUTPUT_BIAS_QUANT),
                ])
                .loss_fn(|output, target| output.sigmoid().squared_error(target))
                .build(|builder, stm_inputs, ntm_inputs| {
                    let l0 = builder.new_affine("l0", 768, HIDDEN_SIZE);
                    let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 1);
                    let stm_hidden = l0.forward(stm_inputs).screlu();
                    let ntm_hidden = l0.forward(ntm_inputs).screlu();
                    l1.forward(stm_hidden.concat(ntm_hidden))
                });
            trainer.run(&schedule, &settings, &loader);
        }
        TrainerFeatureSet::Chess768KingBucketsMirrored3 => {
            let mut trainer = ValueTrainerBuilder::default()
                .seed(NETWORK_SEED)
                .dual_perspective()
                .optimiser(AdamW)
                .inputs(ChessBucketsMirrored::new(KING_BUCKET_LAYOUT_MIRRORED_3))
                .save_format(&[
                    SavedFormat::id("l0w")
                        .transform(|store, weights| {
                            let factoriser =
                                store.get("l0f").values.f32().repeat(KING_BUCKET_COUNT);
                            weights
                                .into_iter()
                                .zip(factoriser)
                                .map(|(bucket, shared)| bucket + shared)
                                .collect()
                        })
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l0b")
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l1w").round().quantise::<i16>(OUTPUT_QUANT),
                    SavedFormat::id("l1b")
                        .round()
                        .quantise::<i32>(OUTPUT_BIAS_QUANT),
                ])
                .loss_fn(|output, target| output.sigmoid().squared_error(target))
                .build(|builder, stm_inputs, ntm_inputs| {
                    let factoriser = builder.new_weights(
                        "l0f",
                        Shape::new(HIDDEN_SIZE, 768),
                        InitSettings::Zeroed,
                    );
                    let mut l0 = builder.new_affine("l0", 768 * KING_BUCKET_COUNT, HIDDEN_SIZE);
                    l0.weights = l0.weights + factoriser.repeat(KING_BUCKET_COUNT);
                    let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 1);
                    let stm_hidden = l0.forward(stm_inputs).screlu();
                    let ntm_hidden = l0.forward(ntm_inputs).screlu();
                    l1.forward(stm_hidden.concat(ntm_hidden))
                });
            let factorised_clip = AdamWParams {
                max_weight: 0.99,
                min_weight: -0.99,
                ..Default::default()
            };
            trainer
                .optimiser
                .set_params_for_weight("l0w", factorised_clip);
            trainer
                .optimiser
                .set_params_for_weight("l0f", factorised_clip);
            trainer.run(&schedule, &settings, &loader);
        }
        TrainerFeatureSet::Chess768KingBucketsMirrored3PhaseHeads4 => {
            let mut trainer = ValueTrainerBuilder::default()
                .seed(NETWORK_SEED)
                .dual_perspective()
                .optimiser(AdamW)
                .inputs(ChessBucketsMirrored::new(KING_BUCKET_LAYOUT_MIRRORED_3))
                .output_buckets(MaterialCount::<PHASE_HEAD_COUNT>)
                .save_format(&[
                    SavedFormat::id("l0w")
                        .transform(|store, weights| {
                            let factoriser =
                                store.get("l0f").values.f32().repeat(KING_BUCKET_COUNT);
                            weights
                                .into_iter()
                                .zip(factoriser)
                                .map(|(bucket, shared)| bucket + shared)
                                .collect()
                        })
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l0b")
                        .round()
                        .quantise::<i16>(ACTIVATION_QUANT),
                    SavedFormat::id("l1w")
                        .transpose()
                        .round()
                        .quantise::<i16>(OUTPUT_QUANT),
                    SavedFormat::id("l1b")
                        .round()
                        .quantise::<i32>(OUTPUT_BIAS_QUANT),
                ])
                .loss_fn(|output, target| output.sigmoid().squared_error(target))
                .build(|builder, stm_inputs, ntm_inputs, output_buckets| {
                    let factoriser = builder.new_weights(
                        "l0f",
                        Shape::new(HIDDEN_SIZE, 768),
                        InitSettings::Zeroed,
                    );
                    let mut l0 = builder.new_affine("l0", 768 * KING_BUCKET_COUNT, HIDDEN_SIZE);
                    l0.weights = l0.weights + factoriser.repeat(KING_BUCKET_COUNT);
                    let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, PHASE_HEAD_COUNT);
                    let stm_hidden = l0.forward(stm_inputs).screlu();
                    let ntm_hidden = l0.forward(ntm_inputs).screlu();
                    l1.forward(stm_hidden.concat(ntm_hidden))
                        .select(output_buckets)
                });
            let factorised_clip = AdamWParams {
                max_weight: 0.99,
                min_weight: -0.99,
                ..Default::default()
            };
            trainer
                .optimiser
                .set_params_for_weight("l0w", factorised_clip);
            trainer
                .optimiser
                .set_params_for_weight("l0f", factorised_clip);
            trainer.run(&schedule, &settings, &loader);
        }
    }
    println!(
        "NEYRANG epoch complete: positions={} checkpoint={}/{}-1",
        plan.positions, args.output_directory, args.net_id
    );
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from(mut values: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut train = None;
    let mut output_directory = None;
    let mut net_id = None;
    let mut positions = None;
    let mut batch_size = None;
    let mut buffer_megabytes = None;
    let mut threads = None;
    let mut position_filter = None;
    let mut wdl_proportion = None;
    let mut feature_set = None;
    let mut input_format = "viriformat".to_string();
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| format!("missing value after {flag}"))?;
        match flag.as_str() {
            "--train" => train = Some(value),
            "--output-dir" => output_directory = Some(value),
            "--net-id" => net_id = Some(value),
            "--positions" => positions = Some(parse_usize(&flag, &value)?),
            "--batch-size" => batch_size = Some(parse_usize(&flag, &value)?),
            "--buffer-mb" => buffer_megabytes = Some(parse_usize(&flag, &value)?),
            "--threads" => threads = Some(parse_usize(&flag, &value)?),
            "--position-filter" => position_filter = Some(value.parse()?),
            "--wdl-proportion" => wdl_proportion = Some(parse_probability(&flag, &value)?),
            "--feature-set" => feature_set = Some(parse_feature_set(&value)?),
            "--input-format" if matches!(value.as_str(), "viriformat" | "teacher-text-v1") => {
                input_format = value;
            }
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    let args = Args {
        train: train.ok_or("missing --train")?,
        output_directory: output_directory.ok_or("missing --output-dir")?,
        net_id: net_id.ok_or("missing --net-id")?,
        positions: positions.ok_or("missing --positions")?,
        batch_size: batch_size.ok_or("missing --batch-size")?,
        buffer_megabytes: buffer_megabytes.ok_or("missing --buffer-mb")?,
        threads: threads.ok_or("missing --threads")?,
        position_filter: position_filter.ok_or("missing --position-filter")?,
        wdl_proportion: wdl_proportion.ok_or("missing --wdl-proportion")?,
        feature_set: feature_set.ok_or("missing --feature-set")?,
        input_format,
    };
    if args.input_format == "teacher-text-v1"
        && (args.position_filter != PositionFilter::None || args.wdl_proportion != 0.0)
    {
        return Err("teacher-text-v1 requires --position-filter none --wdl-proportion 0; export owns eligibility".into());
    }
    Ok(args)
}

fn parse_feature_set(value: &str) -> Result<TrainerFeatureSet, String> {
    match value {
        "chess768" => Ok(TrainerFeatureSet::Chess768),
        "chess768x3hm" => Ok(TrainerFeatureSet::Chess768KingBucketsMirrored3),
        "chess768x3hm4ph" => Ok(TrainerFeatureSet::Chess768KingBucketsMirrored3PhaseHeads4),
        value => Err(format!("unknown feature set: {value}")),
    }
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} requires a positive integer, got {value:?}"))
}

fn parse_probability(flag: &str, value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| format!("{flag} requires a number between 0 and 1, got {value:?}"))?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(format!(
            "{flag} requires a finite number between 0 and 1, got {value:?}"
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{PositionFilter, TrainerFeatureSet, parse_args_from};

    #[test]
    fn teacher_input_is_explicit_and_rejects_incompatible_filter_or_blend() {
        let base = [
            "--train",
            "train.txt",
            "--output-dir",
            "new-output",
            "--net-id",
            "synthetic",
            "--positions",
            "8",
            "--batch-size",
            "4",
            "--buffer-mb",
            "1",
            "--threads",
            "1",
            "--position-filter",
            "none",
            "--wdl-proportion",
            "0",
            "--feature-set",
            "chess768",
        ];
        assert!(parse_args_from(base.map(str::to_owned).into_iter()).is_ok());
        let mut text: Vec<String> = base.into_iter().map(str::to_owned).collect();
        text.extend(["--input-format".into(), "teacher-text-v1".into()]);
        assert!(parse_args_from(text.clone().into_iter()).is_ok());
        for (flag, value) in [
            ("--position-filter", "bullet-default"),
            ("--wdl-proportion", "0.75"),
            ("--input-format", "unknown"),
        ] {
            let mut bad = text.clone();
            let index = bad.iter().position(|v| v == flag).unwrap();
            bad[index + 1] = value.into();
            assert!(parse_args_from(bad.into_iter()).is_err());
        }
    }

    #[test]
    fn trainer_cli_requires_an_explicit_supported_filter_name() {
        let args = parse_args_from(
            [
                "--train",
                "train.vf",
                "--output-dir",
                "checkpoints",
                "--net-id",
                "n2b-filtered",
                "--positions",
                "16008492",
                "--batch-size",
                "9894",
                "--buffer-mb",
                "256",
                "--threads",
                "4",
                "--position-filter",
                "bullet-default",
                "--wdl-proportion",
                "0.0",
                "--feature-set",
                "chess768",
            ]
            .map(str::to_string)
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.position_filter, PositionFilter::BulletDefault);
        assert_eq!(args.wdl_proportion, 0.0);
        assert!(
            parse_args_from(
                ["--position-filter", "mystery"]
                    .map(str::to_string)
                    .into_iter()
            )
            .unwrap_err()
            .contains("unknown position filter")
        );
        assert!(
            parse_args_from(["--wdl-proportion", "1.01"].map(str::to_string).into_iter())
                .unwrap_err()
                .contains("between 0 and 1")
        );
    }

    #[test]
    fn trainer_cli_requires_the_registered_feature_set() {
        let args = parse_args_from(
            [
                "--train",
                "train.vf",
                "--output-dir",
                "checkpoints",
                "--net-id",
                "n2c-king-buckets",
                "--positions",
                "16008492",
                "--batch-size",
                "9894",
                "--buffer-mb",
                "256",
                "--threads",
                "4",
                "--position-filter",
                "none",
                "--wdl-proportion",
                "0.0",
                "--feature-set",
                "chess768x3hm",
            ]
            .map(str::to_string)
            .into_iter(),
        )
        .unwrap();

        assert_eq!(
            args.feature_set,
            TrainerFeatureSet::Chess768KingBucketsMirrored3
        );
        assert!(
            parse_args_from(["--feature-set", "halfka"].map(str::to_string).into_iter())
                .unwrap_err()
                .contains("unknown feature set")
        );
    }
}
