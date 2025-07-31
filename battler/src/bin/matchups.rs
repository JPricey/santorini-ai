use std::{
    collections::HashMap,
};

use santorini_core::{
    board::FullGameState,
    engine::EngineThreadWrapper,
    gods::{ALL_GODS_BY_ID, GodName},
    player::Player,
};

const SECS_PER_MOVE: f32 = 30.0;

fn play_match(engine: &mut EngineThreadWrapper, god1: GodName, god2: GodName) -> Player {
    let mut game_state = FullGameState::new_basic_state(god1, god2);

    loop {
        let Ok(engine_result) = engine.search_for_duration(&game_state, SECS_PER_MOVE) else {
            game_state.print_to_console();
            panic!("could not find a next move");
        };

        eprintln!("{}: score: {} depth: {}", engine_result.action_str, engine_result.score, engine_result.depth);
        game_state = engine_result.child_state;
        game_state.print_to_console();

        if let Some(winner) = game_state.board.get_winner() {
            return winner;
        }
    }
}

#[derive(Debug)]
struct MatchupResult {
    god1: GodName,
    god2: GodName,
    winner: Player,
}

impl MatchupResult {
    fn new(god1: GodName, god2: GodName, winner: Player) -> Self {
        Self { god1, god2, winner }
    }
}

#[derive(Default)]
struct GodResult {
    wins_first: usize,
    wins_second: usize,
    loss_first: usize,
    loss_second: usize,
}

fn print_results(results: &Vec<MatchupResult>) {
    let mut god_results: HashMap<GodName, GodResult> = HashMap::new();
    for r in results {
        let entry1 = god_results.entry(r.god1).or_insert(Default::default());
        if r.winner == Player::One {
            entry1.wins_first += 1;
        } else {
            entry1.loss_first += 1;
        }

        let entry2 = god_results.entry(r.god2).or_insert(Default::default());
        if r.winner == Player::Two {
            entry2.wins_second += 1;
        } else {
            entry2.loss_second += 1;
        }
    }

    eprintln!("All matchup results:");
    for r in results {
        let winner = if r.winner == Player::One {
            r.god1
        } else {
            r.god2
        };

        eprintln!("{} vs {} - {} ({:?})", r.god1, r.god2, winner, r.winner);
    }

    for god in ALL_GODS_BY_ID {
        let god = god.god_name;
        if let Some(entry) = god_results.get(&god) {
            let overall_wins = entry.wins_first + entry.wins_second;
            let overall_loss = entry.loss_first + entry.loss_second;
            eprintln!(
                "{god} - {overall_wins}/{overall_loss} | First: {}/{} | Second: {}/{}",
                entry.wins_first, entry.loss_first, entry.wins_second, entry.loss_second,
            );
        }
    }
}

pub fn main() {
    let mut engine = EngineThreadWrapper::new();

    let mut all_results = Vec::new();

    for god1 in ALL_GODS_BY_ID {
        let god1 = god1.god_name;

        if god1 == GodName::Mortal {
            continue;
        }

        for god2 in ALL_GODS_BY_ID {
            let god2 = god2.god_name;

            if god2 == GodName::Mortal || god1 == god2 {
                continue;
            }

            eprintln!("starting matching {} {}", god1, god2);
            let result = play_match(&mut engine, god1, god2);
            eprintln!("done matching {} {}. Winner: {:?}", god1, god2, result);

            all_results.push(MatchupResult::new(god1, god2, result));
            print_results(&all_results);
        }
    }
}

// cargo run -p battler --bin matchups --release
