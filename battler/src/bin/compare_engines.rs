use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use battler::{BattleResult, WorkerMessage, battling_worker_thread, write_results_to_csv};
use clap::Parser;
use santorini_core::{
    gods::GodName,
    matchup::{Matchup, MatchupSelector},
    player::Player,
    utils::timestamp_string,
};

const DEFAULT_DURATION_SECS: f32 = 0.5;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'e', long)]
    engine1: String,
    #[arg(short = 'E', long)]
    engine2: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut all_matchups = MatchupSelector::default()
        .minus_god_for_both(GodName::Mortal)
        .with_can_swap()
        .with_can_mirror_option(true)
        .get_all();
    all_matchups.push(Matchup::new(GodName::Mortal, GodName::Mortal));
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
            battling_worker_thread::<true>(
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
                if a.winning_player != b.winning_player {
                    let winning_engine = if a.winning_player == Player::One {
                        &a.engine1
                    } else {
                        &a.engine2
                    };
                    let matchup = Matchup::new(a.god1, a.god2);
                    eprintln!(
                        "!!! Matchup {} won on both sides by {}",
                        matchup, winning_engine
                    );
                }
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
// cargo run -p battler --bin compare_engines -r -- -e v111 -E v112 |& tee compare.txt
