use clap::Parser;
use rand::distr::Alphanumeric;
use rand::seq::{IndexedRandom, IteratorRandom};
use rand::{Rng, rng};
use santorini_core::gods::{ALL_GODS_BY_ID, GodName};
use santorini_core::matchup::{Matchup, MatchupSelector};
use santorini_core::placement::{get_starting_placement_state};
use santorini_core::player::Player;
use santorini_core::search::{
    Hueristic, SearchContext, WINNING_SCORE_BUFFER, get_win_reached_search_terminator,
    negamax_search,
};
use santorini_core::search_terminators::{
    AndSearchTerminator, OrSearchTerminator, StaticMaxDepthSearchTerminator,
    StaticNodesVisitedSearchTerminator,
};
use santorini_core::transposition_table::TranspositionTable;
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};

use santorini_core::board::FullGameState;

type DatagenStaticSearchTerminator = OrSearchTerminator<
    StaticNodesVisitedSearchTerminator<100_000>,
    AndSearchTerminator<
        StaticMaxDepthSearchTerminator<8>,
        StaticNodesVisitedSearchTerminator<20_000>,
    >,
>;

#[derive(Debug)]
struct SingleState {
    pub game_state: FullGameState,
    pub score: Hueristic,
    pub calculated_depth: usize,
    pub winner: Player,
    pub move_count: usize,
    pub nodes_visited: usize,
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

const GAMES_PER_FILE: usize = 1_000;

fn worker_thread() {
    let result = _inner_worker_thread();
    match result {
        Ok(_) => eprintln!("Worker thread completed {} games. Exiting", GAMES_PER_FILE),
        Err(e) => eprintln!("Worker thread encountered an error: {:?}", e),
    }
}

fn get_matchups_list() -> Vec<Matchup> {
    // let gods_to_always_generate: Vec<GodName> = WIP_GODS.iter().cloned().collect();
    let gods_to_always_generate: Vec<GodName> =
        [GodName::Zeus, GodName::Ares].iter().cloned().collect();

    let matchup_selector = MatchupSelector::default()
        .with_exact_gods_for_player(Player::One, &gods_to_always_generate)
        .with_can_swap_option(true)
        .with_can_mirror_option(true);

    matchup_selector.get_all()
}

fn _inner_worker_thread() -> Result<(), Box<dyn std::error::Error>> {
    let mut tt = TranspositionTable::new();
    let mut rng = rng();

    let file_path = _get_new_datafile_name(&mut rng);
    let mut data_file = std::fs::File::create(file_path).expect("Failed to create error log file");

    let matchups = get_matchups_list();
    // for m in &matchups {
    //     eprintln!("Including matchup: {}", m);
    // }

    for _ in 0..GAMES_PER_FILE {
        let mut matchup;
        loop {
            matchup = matchups.choose(&mut rng).unwrap().clone();
            if matchup.gods.contains(&GodName::Mortal) && matchup.gods[0] != matchup.gods[1] {
                if rng.random_bool(0.5) {
                    break;
                } else {
                    // 50% chance to reroll mortal vs god matchups
                }
            } else {
                break;
            }
        }
        let now = Instant::now();
        let game_history = generate_one(matchup, &mut tt, &mut rng)?;

        eprintln!(
            "Done single gen. Created {} examples in {:.4}s for {}",
            game_history.len(),
            now.elapsed().as_secs_f32(),
            matchup,
        );

        for game_turn in game_history {
            writeln!(
                data_file,
                "{:?} {} {} {} {} {}",
                game_turn.game_state,
                game_turn.winner as usize + 1,
                game_turn.score,
                game_turn.move_count,
                game_turn.calculated_depth,
                game_turn.nodes_visited,
            )?;
        }

        data_file.flush()?;

        tt.reset();
    }

    Ok(())
}

fn _get_board_with_random_placements(rng: &mut impl Rng) -> FullGameState {
    let mut result = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
    let worker_spots: Vec<usize> = (0..25).choose_multiple(rng, 4).iter().cloned().collect();

    result.board.workers[0].0 |= 1 << worker_spots[0];
    result.board.workers[0].0 |= 1 << worker_spots[1];

    result.board.workers[1].0 |= 1 << worker_spots[2];
    result.board.workers[1].0 |= 1 << worker_spots[3];

    result
}

fn _randomize_gods(state: &mut FullGameState, rng: &mut impl Rng) {
    state.gods[0] = ALL_GODS_BY_ID.choose(rng).unwrap();
    state.gods[1] = ALL_GODS_BY_ID.choose(rng).unwrap();
}

fn playout_subgame(
    rng: &mut impl Rng,
    mut current_state: FullGameState,
    mut move_count: usize,
    tt: &mut TranspositionTable,
    subgame_chance: f64,
) -> Result<Vec<SingleState>, Box<dyn std::error::Error>> {
    let mut game_history: Vec<SingleState> = Vec::new();
    let mut subgame_states: Vec<(FullGameState, usize)> = Vec::new();

    let winner = loop {
        let mut search_context = SearchContext::new(tt, DatagenStaticSearchTerminator::default());

        let search_result = negamax_search::<DatagenStaticSearchTerminator>(
            &mut search_context,
            current_state.clone(),
            get_win_reached_search_terminator(),
        );

        let Some(best_child) = search_result.best_move else {
            eprint!("Search returned no results for state {:?}", current_state);

            for game_turn in game_history {
                eprintln!(
                    "{:?} {} {} {} {} {}",
                    game_turn.game_state,
                    game_turn.winner as usize + 1,
                    game_turn.score,
                    game_turn.move_count,
                    game_turn.calculated_depth,
                    game_turn.nodes_visited,
                );
            }

            return Err("Search returned no results".to_owned().into());
        };

        if let Some(winner) = best_child.child_state.board.get_winner() {
            break winner;
        } else {
            game_history.push(SingleState {
                game_state: current_state.clone(),
                score: best_child.score,
                calculated_depth: best_child.depth,
                winner: Player::One,
                nodes_visited: search_result.nodes_visited,
                move_count,
            });

            if best_child.score.abs() < WINNING_SCORE_BUFFER {
                if rng.random_bool(subgame_chance) {
                    let random_next = current_state.get_next_states().choose(rng).unwrap().clone();
                    if random_next != best_child.child_state {
                        subgame_states.push((random_next, move_count));
                    }
                }
            }
        }

        current_state = best_child.child_state.clone();
        move_count += 1;
    };

    for item in &mut game_history {
        item.winner = winner;
    }

    for (substate, sub_movecount) in subgame_states {
        let mut child_states =
            playout_subgame(rng, substate, sub_movecount, tt, subgame_chance / 2.0)?;
        game_history.append(&mut child_states);
    }

    Ok(game_history)
}

fn generate_one(
    matchup: Matchup,
    tt: &mut TranspositionTable,
    rng: &mut impl Rng,
) -> Result<Vec<SingleState>, Box<dyn std::error::Error>> {
    let mut current_state = FullGameState::new_for_matchup(&matchup);
    let mut move_count = 0;

    for _ in 0..2 {
        let placement_mode = get_starting_placement_state(&current_state.board, current_state.gods)
            .unwrap()
            .unwrap();

        let active_player = placement_mode.next_placement;
        let active_god = current_state.gods[active_player as usize];
        let placement_actions = active_god.get_all_placement_actions(
            current_state.gods,
            &current_state.board,
            active_player,
        );
        let action = placement_actions.choose(rng).unwrap().clone();

        active_god.make_placement_move(action, &mut current_state.board, active_player);
    }

    eprintln!("Random starting state: {:?}", current_state);

    let rand = rng.random_range(0.0..1.0);
    let num_random_moves = match rand {
        ..0.01 => 2,
        ..0.1 => 3,
        ..0.3 => 4,
        ..0.6 => 5,
        _ => 6,
    };

    for _ in 0..num_random_moves {
        let child_states = current_state.get_next_states();
        current_state = child_states
            .choose(rng)
            .ok_or("Failed to find random child")?
            .clone();
        move_count += 1;

        if current_state.get_winner().is_some() {
            return Ok(vec![]); // Early end, no data to record
        }
    }

    playout_subgame(rng, current_state, move_count, tt, 0.3)
}

#[derive(Parser, Debug)]
struct DatagenArgs {
    #[arg(short = 'j', long)]
    pub threads: Option<usize>,
}

pub fn main() {
    let args = DatagenArgs::parse();

    while std::fs::create_dir_all(&_gamedata_directory()).is_err() {
        eprintln!("Failed to create data logs directory... Trying again.");
        sleep(Duration::from_millis(500));
    }

    let num_cpus = num_cpus::get();
    let num_worker_threads = args.threads.unwrap_or_else(|| std::cmp::max(1, num_cpus));
    println!("Found {num_cpus} CPUs. Creating {num_worker_threads} threads",);

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

// cargo run -p datagen -r
// Specific threadcount:
// cargo run -p datagen -r -- -j 8
