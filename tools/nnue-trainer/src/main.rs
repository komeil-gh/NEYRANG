use std::{env, path::Path, process::ExitCode};

use neyrang_nnue_trainer::{DeterministicViriLoader, EpochPlan, POSITION_SHUFFLE_SEED};
use bullet::{
    game::{formats::bulletformat::ChessBoard, inputs::Chess768},
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::ValueTrainerBuilder,
};

const HIDDEN_SIZE: usize = 128;
const EVAL_SCALE: i32 = 400;
const ACTIVATION_QUANT: i16 = 255;
const OUTPUT_QUANT: i16 = 64;
const INITIAL_LR: f32 = 0.001;
const WDL_PROPORTION: f32 = 0.75;
const NETWORK_SEED: u64 = 20_260_901;

#[derive(Debug)]
struct Args {
    train: String,
    output_directory: String,
    net_id: String,
    positions: usize,
    batch_size: usize,
    buffer_megabytes: usize,
    threads: usize,
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
                .quantise::<i32>(i32::from(ACTIVATION_QUANT) * i32::from(OUTPUT_QUANT)),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs| {
            let l0 = builder.new_affine("l0", 768, HIDDEN_SIZE);
            let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 1);
            let stm_hidden = l0.forward(stm_inputs).screlu();
            let ntm_hidden = l0.forward(ntm_inputs).screlu();
            l1.forward(stm_hidden.concat(ntm_hidden))
        });

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
            value: WDL_PROPORTION,
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
    let chunk_positions = args
        .buffer_megabytes
        .checked_mul(1024 * 1024)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<ChessBoard>()))
        .ok_or("loader buffer size overflow")?;
    let loader = DeterministicViriLoader::new(&args.train, chunk_positions)
        .map_err(|error| error.to_string())?;

    println!(
        "NEYRANG epoch contract: positions={} batch_size={} batches={} network_seed={} position_shuffle_seed={} bullet_rev=629ee50000b2afb7b3337595401c830d3b1e0f42",
        plan.positions, plan.batch_size, plan.batches, NETWORK_SEED, POSITION_SHUFFLE_SEED,
    );
    trainer.run(&schedule, &settings, &loader);
    println!(
        "NEYRANG epoch complete: positions={} checkpoint={}/{}-1",
        plan.positions, args.output_directory, args.net_id
    );
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut values = env::args().skip(1);
    let mut train = None;
    let mut output_directory = None;
    let mut net_id = None;
    let mut positions = None;
    let mut batch_size = None;
    let mut buffer_megabytes = None;
    let mut threads = None;
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
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    Ok(Args {
        train: train.ok_or("missing --train")?,
        output_directory: output_directory.ok_or("missing --output-dir")?,
        net_id: net_id.ok_or("missing --net-id")?,
        positions: positions.ok_or("missing --positions")?,
        batch_size: batch_size.ok_or("missing --batch-size")?,
        buffer_megabytes: buffer_megabytes.ok_or("missing --buffer-mb")?,
        threads: threads.ok_or("missing --threads")?,
    })
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} requires a positive integer, got {value:?}"))
}
