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
    utils::timestamp_string,
};

const DEFAULT_DURATION_SECS: f32 = 1.0;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'e', long)]
    engine1: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,
}

pub fn get_all_matchups() -> Vec<Matchup> {
    let all_matchups = MatchupSelector::default()
        .minus_god_for_both(GodName::Mortal)
        .with_can_mirror_option(true)
        .with_can_swap()
        //.with_exact_gods_for_player(
        //    Player::One,
        //    vec![
        //        GodName::Urania,
        //        GodName::Graeae,
        //        GodName::Hera,
        //        GodName::Limus,
        //        GodName::Hypnus,
        //        GodName::Harpies,
        //    ],
        //)
        .get_all();

    // for m in &all_matchups {
    //     eprintln!("matchup: {}", m);
    // }

    all_matchups
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut all_matchups = get_all_matchups();
    all_matchups.sort();
    all_matchups.reverse();
    for m in &all_matchups {
        eprintln!("{m}");
    }
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
        let duration = Duration::from_secs_f32(args.secs);
        std::thread::spawn(move || {
            battling_worker_thread::<false>(
                worker_idx.to_string(),
                matchups_queue,
                &engine1,
                &engine1,
                duration,
                tx.clone(),
            );
            // Sleep a bit to make sure we don't miss anything
            std::thread::sleep(Duration::from_secs(1));
            tx.send(WorkerMessage::Done).unwrap();

            // HACK: sleep forever to not cause a panic, because we didn't implement
            // clean shutdowns for worker threads
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }

    eprintln!("starting {}", timestamp_string());

    loop {
        let msg = rx.recv()?;
        match msg {
            WorkerMessage::BattleResult(result) => {
                eprintln!("{}", result.get_pretty_description());
                all_results.push(result.clone());
                write_results_to_csv(&all_results, &PathBuf::from("tmp/all_matchups.csv"))?;

                eprintln!(
                    "{} reported: {}/{}",
                    timestamp_string(),
                    all_results.len(),
                    matchups_count
                );
            }
            WorkerMessage::BattleResultPair(_) => {
                unreachable!();
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
// cargo run -p battler --bin run_matchups -r -- -e v102
