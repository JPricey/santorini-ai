use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread::{self},
    time::Duration,
};

use battler::{
    BattleResult, WorkerMessage, create_tmp_dir, read_battle_result_csv, write_results_to_csv,
};
use clap::Parser;
use santorini_core::{
    board::FullGameState,
    engine::EngineThreadWrapper,
    matchup::{Matchup, MatchupArgs},
    utils::timestamp_string,
};

const DEFAULT_DURATION_SECS: f32 = 4.0;
const MATCHUPS_CSV_FILE: &str = "tmp/all_matchups.csv";

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,

    #[arg(short = 'c', long, default_value_t = false)]
    cont: bool,

    #[command(flatten)]
    matchups: MatchupArgs,
}

fn _read_battle_results_csv() -> Vec<BattleResult> {
    read_battle_result_csv(&PathBuf::from(MATCHUPS_CSV_FILE)).unwrap()
}

fn get_all_matchups(args: &Args) -> Vec<Matchup> {
    args.matchups.to_selector().get_all()
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

        let thread_name = {
            let current_thread = thread::current();
            current_thread.name().unwrap_or("unknown").to_string()
        };

        eprintln!(
            "{}: starting matchup {} {}",
            thread_name,
            timestamp_string(),
            next_matchup
        );

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
    create_tmp_dir();
    let args = Args::parse();

    let mut all_matchups = get_all_matchups(&args);
    let mut all_results = Vec::<BattleResult>::new();

    if args.cont {
        eprintln!("Cont mode - continue from previous run");
        let completed_results = read_battle_result_csv(&PathBuf::from(MATCHUPS_CSV_FILE))?;
        all_results = completed_results.clone();

        let completed_matchups: Vec<Matchup> = completed_results
            .iter()
            .map(|res| Matchup::new(res.god1, res.god2))
            .collect();

        all_matchups.retain(|m| !completed_matchups.contains(m));
    }
    let base_results = all_results.len();

    all_matchups.sort();
    all_matchups.reverse();
    for m in &all_matchups {
        eprintln!("{m}");
    }
    let matchups_count = all_matchups.len();

    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    let num_cpus = num_cpus::get();
    let num_workers = num_cpus - 1;

    eprintln!("Starting {} workers", num_workers);

    let all_matchups_queue = Arc::new(Mutex::new(all_matchups));

    let mut done_workers_count = 0;
    for i in 0..num_workers {
        let tx = tx.clone();
        let matchups_queue = Arc::clone(&all_matchups_queue);

        let duration = Duration::from_secs_f32(args.secs);
        thread::Builder::new()
            .name(format!("thread_{i}"))
            .spawn(move || {
                worker_thread(matchups_queue, duration, tx.clone());
                // Sleep a bit to make sure we don't miss anything
                std::thread::sleep(Duration::from_secs(1));
                tx.send(WorkerMessage::Done).unwrap();
            })
            .expect(format!("failed to spawn thread {}", i).as_str());
    }

    eprintln!("starting {}", timestamp_string());

    loop {
        let msg = rx.recv()?;
        match msg {
            WorkerMessage::BattleResult(result) => {
                eprintln!("{}", result.get_pretty_description());
                all_results.push(result.clone());
                write_results_to_csv(&all_results, &PathBuf::from(MATCHUPS_CSV_FILE))?;

                eprintln!(
                    "{} reported: {}/{}",
                    timestamp_string(),
                    all_results.len(),
                    matchups_count + base_results,
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
// cargo run -p battler --bin run_matchups -r -- -s 10.0
// cargo run -p battler --bin run_matchups -r -- --p1 chronus
// cargo run -p battler --bin run_matchups -r -- --p1 chronus --p2 athena
// cargo run -p battler --bin run_matchups -r -- --p1 medusa iris castor
// cargo run -p battler --bin run_matchups -r -- --exclude atlas selene --no-mirror
// cargo run -p battler --bin run_matchups -r -- --p1 chronus -c
