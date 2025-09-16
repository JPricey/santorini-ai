use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use battler::{BattleResult, WorkerMessage, write_results_to_csv};
use clap::Parser;
use santorini_core::{
    board::FullGameState,
    engine::EngineThreadWrapper,
    gods::GodName,
    matchup::{Matchup, MatchupSelector},
    utils::timestamp_string,
};

const DEFAULT_DURATION_SECS: f32 = 10.0;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,
}

pub fn get_all_matchups() -> Vec<Matchup> {
    let all_matchups = MatchupSelector::default()
        .minus_god_for_both(GodName::Mortal)
        .with_can_mirror_option(true)
        .with_can_swap()
        // .with_exact_gods_for_player(
        //     santorini_core::player::Player::One,
        //     &santorini_core::gods::WIP_GODS,
        // )
        .with_exact_gods_for_player(santorini_core::player::Player::One, &[GodName::Europa])
        .get_all();

    // for m in &all_matchups {
    //     eprintln!("matchup: {}", m);
    // }

    all_matchups
}

fn worker_thread(
    matchups_queue: Arc<Mutex<Vec<Matchup>>>,
    duration: Duration,
    result_channel: mpsc::Sender<WorkerMessage>,
) {
    let mut engine = EngineThreadWrapper::new();
    engine.spin_for_pending_state();

    loop {
        let next_matchup = {
            let mut queue = matchups_queue.lock().unwrap();
            queue.pop()
        };
        let Some(next_matchup) = next_matchup else {
            engine.end();
            break;
        };

        let root_state = FullGameState::new_for_matchup(&next_matchup);
        let battle_result = playout_game(&root_state, &mut engine, duration).unwrap();
        result_channel
            .send(WorkerMessage::BattleResult(battle_result))
            .unwrap();
    }
}

fn playout_game(
    root_state: &FullGameState,
    engine: &mut EngineThreadWrapper,
    duration: Duration,
) -> Result<BattleResult, String> {
    let mut current_state = root_state.clone();

    let mut moves_made = 0;
    loop {
        if let Some(winner) = current_state.get_winner() {
            return Ok(BattleResult {
                god1: root_state.gods[0].god_name,
                god2: root_state.gods[1].god_name,
                engine1: "latest".to_string(),
                engine2: "latest".to_string(),
                winning_player: winner,
                moves_made,
            });
        }

        let best_move = engine
            .search_for_duration(&current_state, duration.as_secs_f32())
            .map_err(|err| format!("Error in search on state: {:?}, {:?}", current_state, err))?;
        current_state = best_move.child_state;
        moves_made += 1;
    }
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
    let num_workers = num_cpus - 1;

    eprintln!("Starting {} workers", num_workers);

    let all_matchups_queue = Arc::new(Mutex::new(all_matchups));

    let mut done_workers_count = 0;
    for _ in 0..num_workers {
        let tx = tx.clone();
        let matchups_queue = Arc::clone(&all_matchups_queue);

        let duration = Duration::from_secs_f32(args.secs);
        std::thread::spawn(move || {
            worker_thread(matchups_queue, duration, tx.clone());
            // Sleep a bit to make sure we don't miss anything
            std::thread::sleep(Duration::from_secs(1));
            tx.send(WorkerMessage::Done).unwrap();

            // HACK: sleep forever to not cause a panic, because we didn't implement
            // clean shutdowns for worker threads
            // loop {
            //     std::thread::sleep(Duration::from_secs(1));
            // }
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
// cargo run -p battler --bin run_matchups -r
