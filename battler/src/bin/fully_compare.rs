use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use battler::{BINARY_DIRECTORY, BattleResult, do_battle, prepare_subprocess};
use clap::Parser;
use csv::Writer;
use santorini_core::{
    board::FullGameState,
    matchup::{Matchup, MatchupSelector},
    utils::timestamp_string,
};

const DEFAULT_DURATION_SECS: f32 = 1.0;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'e', long)]
    engine1: String,
    #[arg(short = 'E', long)]
    engine2: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,
}

fn write_results_to_csv(results: &[BattleResult], path: &PathBuf) -> std::io::Result<()> {
    let mut wtr = Writer::from_path(path)?;
    for result in results {
        wtr.serialize(result)?;
    }
    wtr.flush()?;
    Ok(())
}

fn battling_worker(
    worker_idx: String,
    matchups_queue: Arc<Mutex<Vec<Matchup>>>,
    engine1: &PathBuf,
    engine2: &PathBuf,
    duration: Duration,
    result_channel: mpsc::Sender<WorkerMessage>,
) {
    let now_str = timestamp_string();
    let mut c1 = prepare_subprocess(
        &PathBuf::from(format!(
            "compare-{worker_idx}-{}-{}.log",
            now_str,
            engine1.display()
        )),
        &PathBuf::new().join(BINARY_DIRECTORY).join(engine1),
    );
    let mut c2 = prepare_subprocess(
        &PathBuf::from(format!(
            "compare-{worker_idx}-{}-{}.log",
            now_str,
            engine2.display()
        )),
        &PathBuf::new().join(BINARY_DIRECTORY).join(engine2),
    );

    loop {
        let matchup = {
            let mut queue = matchups_queue.lock().unwrap();
            queue.pop()
        };

        match matchup {
            Some(matchup) => {
                let start_state = FullGameState::new_for_matchup(&matchup);

                let result1 = do_battle(&start_state, &mut c1, &mut c2, duration, false);
                let result2 = do_battle(&start_state, &mut c2, &mut c1, duration, false);

                result_channel
                    .send(WorkerMessage::BattleResult(result1))
                    .unwrap();
                result_channel
                    .send(WorkerMessage::BattleResult(result2))
                    .unwrap();
            }
            None => {
                break;
            }
        }
    }
}

pub enum WorkerMessage {
    BattleResult(BattleResult),
    BattleResultPair((BattleResult, BattleResult)),
    Done,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut all_matchups = MatchupSelector::default()
        .with_can_swap()
        .with_can_mirror_option(true)
        .get_all();
    all_matchups.sort();
    all_matchups.reverse();
    let matchups_count = all_matchups.len();

    let mut all_results = Vec::<BattleResult>::new();
    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    let num_cpus = num_cpus::get();
    let num_workers = num_cpus / 2;

    eprintln!("Starting {} workers", num_workers);

    let all_matchups_queue = Arc::new(Mutex::new(all_matchups));

    let mut done_workers_count = 0;
    for worker_idx in 0..num_workers {
        let tx = tx.clone();
        let matchups_queue = Arc::clone(&all_matchups_queue);
        let engine1 = PathBuf::from(&args.engine1);
        let engine2 = PathBuf::from(&args.engine2);
        let duration = Duration::from_secs_f32(args.secs);
        std::thread::spawn(move || {
            battling_worker(
                worker_idx.to_string(),
                matchups_queue,
                &engine1,
                &engine2,
                duration,
                tx.clone(),
            );
            // Sleep a bit to make sure we don't miss anything
            std::thread::sleep(Duration::from_secs(1));
            tx.send(WorkerMessage::Done).unwrap();
        });
    }

    eprintln!("starting {}", timestamp_string());

    loop {
        let msg = rx.recv()?;
        match msg {
            WorkerMessage::BattleResult(result) => {
                eprintln!("{}", result.get_pretty_description());
                all_results.push(result.clone());
                write_results_to_csv(&all_results, &PathBuf::from("tmp/engine_cmp.csv"))?;

                eprintln!(
                    "{} reported: {}/{}",
                    timestamp_string(),
                    all_results.len(),
                    matchups_count * 2
                );
            }
            WorkerMessage::BattleResultPair((a, b)) => {
                eprintln!("{}", a.get_pretty_description());
                eprintln!("{}", b.get_pretty_description());
                all_results.push(a.clone());
                all_results.push(b.clone());
                write_results_to_csv(&all_results, &PathBuf::from("tmp/engine_cmp.csv"))?;

                eprintln!(
                    "{} reported: {}/{}",
                    timestamp_string(),
                    all_results.len(),
                    matchups_count * 2
                );
            }
            WorkerMessage::Done => {
                done_workers_count += 1;
                if done_workers_count >= num_workers {
                    break;
                }
            }
        }
    }

    Ok(())
}
// cargo run -p battler --bin fully_compare -r -- -e v101 -E v102
