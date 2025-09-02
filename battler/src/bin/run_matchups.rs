use std::collections::HashMap;

use santorini_core::{
    board::FullGameState,
    engine::EngineThreadWrapper,
    gods::{ALL_GODS_BY_ID, GodName},
    matchup::{Matchup, MatchupSelector},
    nnue::SCALE,
    player::Player,
    search::Hueristic,
    utils::sigmoid,
};

const SECS_PER_MOVE: f32 = 1.0;

fn play_match(engine: &mut EngineThreadWrapper, matchup: &Matchup) -> Player {
    let mut game_state = FullGameState::new_for_matchup(&matchup);

    loop {
        let Ok(engine_result) = engine.search_for_duration(&game_state, SECS_PER_MOVE) else {
            game_state.print_to_console();
            panic!("could not find a next move");
        };

        eprintln!(
            "{}: score: {} depth: {}",
            engine_result.action_str, engine_result.score, engine_result.depth
        );
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

pub fn all_matchups() -> Vec<Matchup> {
    let all_matchups = MatchupSelector::default()
        .minus_god_for_both(GodName::Mortal)
        .with_can_mirror_option(false)
        .with_can_swap()
        .with_exact_gods_for_player(
            Player::One,
            vec![
                GodName::Urania,
                GodName::Graeae,
                GodName::Hera,
                GodName::Limus,
                GodName::Hypnus,
                GodName::Harpies,
            ],
        )
        .get_all();

    // for m in &all_matchups {
    //     eprintln!("matchup: {}", m);
    // }

    all_matchups
}

pub fn full_matchups() {
    let mut engine = EngineThreadWrapper::new();
    engine.spin_for_pending_state();

    let mut all_results = Vec::new();

    for matchup in all_matchups() {
        eprintln!("starting match: {}", matchup);
        let result = play_match(&mut engine, &matchup);
        let winning_god = matchup.gods[result as usize];
        eprintln!("done match {}. Winner: {:?} - {:?}", matchup, result, winning_god);

        all_results.push(MatchupResult::new(
            matchup.god_1().god_name,
            matchup.god_2().god_name,
            result,
        ));
        print_results(&all_results);
    }
}

#[derive(Debug)]
struct BalanceMatchupResult {
    god1: GodName,
    god2: GodName,
    scores: Vec<Hueristic>,
    average_score: f32,
}

impl std::fmt::Display for BalanceMatchupResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}, {}, {:.2}, {:.2}, {:?}",
            self.god1,
            self.god2,
            self.average_score,
            100.0 * sigmoid(self.average_score / SCALE as f32),
            self.scores
        )
    }
}

/// Outputs how balanced the AI thinks each matchup is by playing out a few turns and returning the
/// score
/// Used to determine which gods to pick for an even match
pub fn balance_matchups(moves_per_game: usize, secs_per_move: f32) {
    let mut engine = EngineThreadWrapper::new();
    engine.spin_for_pending_state();

    let mut all_results = Vec::new();
    let all_matchups = MatchupSelector::default()
        .with_can_mirror_option(false)
        .get_all();
    for m in &all_matchups {
        eprintln!("matchup: {}", m);
    }

    for matchup in &all_matchups {
        eprintln!("starting matchup {}", matchup);
        let mut scores = Vec::new();

        let mut game_state = FullGameState::new_for_matchup(matchup);
        for _ in 0..moves_per_game {
            let engine_result = engine
                .search_for_duration(&game_state, secs_per_move)
                .unwrap();
            let sign = match game_state.board.current_player {
                Player::One => 1,
                Player::Two => -1,
            };
            scores.push(sign * engine_result.score);

            eprintln!(
                "{}: score: {} depth: {}",
                engine_result.action_str, engine_result.score, engine_result.depth
            );
            game_state = engine_result.child_state;
            game_state.print_to_console();
        }

        let average_score =
            scores.iter().cloned().map(f32::from).sum::<f32>() / scores.len() as f32;
        all_results.push(BalanceMatchupResult {
            god1: matchup.god_1().god_name,
            god2: matchup.god_2().god_name,
            scores,
            average_score,
        });

        for bit in &all_results {
            eprintln!("{bit}");
        }
    }
}

pub fn main() {
    // balance_matchups(4, 5.0);
    full_matchups()
}

// cargo run -p battler --bin matchups --release
