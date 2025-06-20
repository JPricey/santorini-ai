use rand::distributions::Alphanumeric;
use rand::seq::IteratorRandom;
use rand::{Rng, seq::SliceRandom, thread_rng};
use santorini_core::gods::GodName;
use santorini_core::transposition_table::TranspositionTable;
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};

use santorini_core::board::{FullGameState, Player};
use santorini_core::search::{Hueristic, SearchContext, StaticSearchTerminator, search_with_state};

const MIN_NUM_RANDOM_MOVES: usize = 4;

pub struct DatagenStaticSearchTerminator {}

impl StaticSearchTerminator for DatagenStaticSearchTerminator {
    fn should_stop(search_state: &santorini_core::search::SearchState) -> bool {
        search_state.last_fully_completed_depth >= 7 && search_state.nodes_visited > 1_000_000
    }
}

#[derive(Debug)]
struct SingleState {
    pub game_state: FullGameState,
    pub score: Hueristic,
    pub calculated_depth: usize,
    pub winner: Player,
}

fn _gamedata_directory() -> PathBuf {
    let mut path = std::env::current_dir().expect("Failed to get current directory");
    path.push("game_data");
    path
}

fn _get_new_datafile_name(rng: &mut impl Rng) -> PathBuf {
    let random_name: String = rng
        .sample_iter(&Alphanumeric)
        .take(15)
        .map(char::from)
        .collect();

    let mut path = _gamedata_directory();
    path.push(format!("gamedata-{}.txt", random_name));

    path
}

fn worker_thread() {
    let result = _inner_worker_thread();
    if let Err(e) = result {
        eprintln!("Worker thread encountered an error. Aborting: {:?}", e);
    }
}

fn _inner_worker_thread() -> Result<(), Box<dyn std::error::Error>> {
    let mut tt = TranspositionTable::new();
    let mut rng = thread_rng();

    let file_path = _get_new_datafile_name(&mut rng);
    let mut data_file = std::fs::File::create(file_path).expect("Failed to create error log file");

    loop {
        let now = Instant::now();
        let game_history = generate_one(&mut tt, &mut rng)?;

        eprintln!(
            "Done single gen. Created {} examples in {:.4}s",
            game_history.len(),
            now.elapsed().as_secs_f32()
        );

        for game_turn in game_history {
            // eprintln!(
            //     "{:?} {} {} {}",
            //     game_turn.game_state,
            //     game_turn.winner as usize + 1,
            //     game_turn.calculated_depth,
            //     game_turn.score
            // );

            writeln!(
                data_file,
                "{:?} {} {} {}",
                game_turn.game_state,
                game_turn.winner as usize + 1,
                game_turn.calculated_depth,
                game_turn.score
            )?;
        }

        data_file.flush()?;

        tt.reset();
    }
}

fn _get_board_with_random_placements(rng: &mut impl Rng) -> FullGameState {
    let mut result = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
    let worker_spots: Vec<usize> = (0..25).choose_multiple(rng, 4).iter().cloned().collect();

    result.board.workers[0] |= 1 << worker_spots[0];
    result.board.workers[0] |= 1 << worker_spots[1];

    result.board.workers[1] |= 1 << worker_spots[2];
    result.board.workers[1] |= 1 << worker_spots[3];

    result
}

fn generate_one(
    tt: &mut TranspositionTable,
    rng: &mut impl Rng,
) -> Result<Vec<SingleState>, Box<dyn std::error::Error>> {
    let mut game_history = Vec::new();

    let mut current_state = _get_board_with_random_placements(rng);

    game_history.push(SingleState {
        game_state: current_state.clone(),
        score: 0,
        calculated_depth: 0,
        winner: Player::One,
    });

    for _ in 0..MIN_NUM_RANDOM_MOVES {
        let child_states = current_state.get_next_states();
        current_state = child_states
            .choose(rng)
            .ok_or("Failed to find random child")?
            .clone();
    }

    if rng.gen_bool(0.75) {
        let child_states = current_state.get_next_states();
        current_state = child_states
            .choose(rng)
            .ok_or("Failed to find random child")?
            .clone();
    }

    let winner = loop {
        let mut search_context = SearchContext::new(tt);

        let search_result =
            search_with_state::<DatagenStaticSearchTerminator>(&mut search_context, &current_state);

        let best_child = search_result.best_move.ok_or("Search returned no moves")?;

        if let Some(winner) = best_child.state.board.get_winner() {
            break winner;
        } else {
            game_history.push(SingleState {
                game_state: current_state.clone(),
                score: best_child.score,
                calculated_depth: best_child.depth,
                winner: Player::One,
            });
        }

        current_state = best_child.state.clone();
    };

    for item in &mut game_history {
        item.winner = winner;
    }

    // eprint!("{:?}", tt);
    Ok(game_history)
}

pub fn main() {
    while std::fs::create_dir_all(&_gamedata_directory()).is_err() {
        eprintln!("Failed to create data logs directory... Trying again.");
        sleep(Duration::from_millis(500));
    }

    let num_cpus = num_cpus::get();
    let num_worker_threads = std::cmp::max(1, num_cpus - 1);
    println!(
        "Found {} CPUs. Using {} worker threads",
        num_cpus, num_worker_threads
    );

    let mut worker_threads = Vec::new();

    for _ in 0..num_worker_threads {
        let new_thread = thread::spawn(&worker_thread);
        worker_threads.push(new_thread);
    }

    loop {
        for i in 0..num_worker_threads {
            if worker_threads[i].is_finished() {
                eprintln!("Worker thread {i} has died. Recreating.");
                let new_thread = thread::spawn(&worker_thread);
                worker_threads.push(new_thread);
                worker_threads.remove(i);
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}
