use std::time::Instant;

use clap::Parser;
use rand::{Rng, rng, seq::IndexedRandom};

use santorini_core::{
    board::FullGameState,
    consistency_checker::consistency_check,
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
    matchup::MatchupSelector,
    player::Player,
    random_utils::{get_random_starting_state, get_random_state_flattening_powers},
};

fn run_match_return_should_continue(root_state: FullGameState, rng: &mut impl Rng) -> bool {
    let mut prev_state = root_state.clone();
    let mut current_state = root_state;
    loop {
        if current_state.board.get_winner().is_some() {
            return true;
        }

        if let Err(err) = consistency_check(&current_state) {
            eprintln!("Consistency check failed: {:?}", current_state);
            eprintln!("Previous state: {:?}", prev_state);
            current_state.print_to_console();

            for error_line in err {
                eprintln!("{error_line}");
            }
            return false;
        }

        if let Some(next_state) = get_random_state_flattening_powers(&current_state, rng) {
            prev_state = current_state;
            current_state = next_state;
        } else {
            return true;
        }
    }
}

struct GodRandomizer {
    gods: Vec<StaticGod>,
}

#[allow(dead_code)]
impl GodRandomizer {
    pub fn new_any() -> Self {
        Self {
            gods: ALL_GODS_BY_ID.iter().collect(),
        }
    }

    pub fn new_exactly(god: GodName) -> Self {
        Self {
            gods: vec![god.to_power()],
        }
    }

    pub fn new_one_of<I: Iterator<Item = GodName>>(gods: I) -> Self {
        Self {
            gods: gods.map(|n| n.to_power()).collect(),
        }
    }

    pub fn new_not_one_of(gods: &Vec<GodName>) -> Self {
        Self::new_one_of(
            ALL_GODS_BY_ID
                .map(|g| g.god_name)
                .iter()
                .filter(|g| !gods.contains(g))
                .cloned(),
        )
    }

    pub fn get(&self) -> StaticGod {
        let mut rng = rng();
        self.gods.choose(&mut rng).unwrap()
    }
}

#[derive(Debug, Parser)]
struct FuzzerArgs {
    #[arg(short = 'g', long, num_args=0.., value_delimiter=' ')]
    p1_gods: Vec<GodName>,

    #[arg(short = 'G', long, num_args=0.., value_delimiter=' ')]
    p2_gods: Vec<GodName>,

    /// Stop the fuzzer on the first failure.
    #[arg(short = 's', long)]
    stop_on_failure: bool,

    /// Stop running after this many seconds.
    #[arg(short = 't', long)]
    timeout_secs: Option<f64>,
}

fn maybe_kill_random_worker<T: Rng>(state: &mut FullGameState, player: Player, rng: &mut T) {
    if rng.random_bool(0.9) {
        return;
    }

    let worker_squares = state.board.workers[player as usize].all_squares();
    if let Some(square) = worker_squares.choose(rng) {
        state
            .board
            .oppo_worker_kill(state.gods[player as usize], player, square.to_board());
    }
}

fn main() {
    let mut rng = rng();
    let args = FuzzerArgs::parse();

    let deadline = args
        .timeout_secs
        .map(|secs| Instant::now() + std::time::Duration::from_secs_f64(secs));

    let mut matchup_selector = MatchupSelector::default().with_can_swap();
    if args.p1_gods.len() > 0 {
        matchup_selector = matchup_selector.with_exact_gods_for_player(Player::One, &args.p1_gods);
    }
    if args.p2_gods.len() > 0 {
        matchup_selector = matchup_selector.with_exact_gods_for_player(Player::Two, &args.p2_gods);
    }

    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            eprintln!("Timeout reached, stopping fuzzer.");
            break;
        }

        let mut matchup = matchup_selector.get();
        if rng.random_bool(0.5) {
            matchup = matchup.flip();
        }

        let mut root_state = get_random_starting_state(&matchup, &mut rng);

        maybe_kill_random_worker(&mut root_state, Player::One, &mut rng);
        maybe_kill_random_worker(&mut root_state, Player::Two, &mut rng);

        if root_state.validation_err().is_err() {
            continue;
        }

        let should_continue = run_match_return_should_continue(root_state, &mut rng);
        if args.stop_on_failure && !should_continue {
            eprintln!("Failure detected, stopping fuzzer.");
            break;
        }
    }
}

// cargo run -p santorini_core --bin fuzzer -r
// cargo run -p santorini_core --bin fuzzer -r -- -g morpheus
