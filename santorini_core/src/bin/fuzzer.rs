use clap::Parser;
use rand::{Rng, rng, seq::IndexedRandom};

use santorini_core::{
    board::FullGameState,
    consistency_checker::consistency_check,
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
    matchup::MatchupSelector,
    player::Player,
    random_utils::{get_board_with_random_placements_worker_counters, get_random_move},
};

fn run_match(root_state: FullGameState, rng: &mut impl Rng) {
    let mut current_state = root_state;
    loop {
        if current_state.board.get_winner().is_some() {
            return;
        }

        if let Err(err) = consistency_check(&current_state) {
            eprintln!("Consistency check failed: {:?}", current_state);
            current_state.print_to_console();

            for error_line in err {
                eprintln!("{error_line}");
            }
            return;
            // panic!("Consistency check failed");
        }

        if let Some(next_state) = get_random_move(&current_state, rng) {
            current_state = next_state;
        } else {
            return;
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
}

fn main() {
    let mut rng = rng();
    let args = FuzzerArgs::parse();

    let mut matchup_selector = MatchupSelector::default().with_can_swap();
    if args.p1_gods.len() > 0 {
        matchup_selector = matchup_selector.with_exact_gods_for_player(Player::One, &args.p1_gods);
    }
    if args.p2_gods.len() > 0 {
        matchup_selector = matchup_selector.with_exact_gods_for_player(Player::Two, &args.p2_gods);
    }

    loop {
        let mut matchup = matchup_selector.get();
        if rng.random_bool(0.5) {
            matchup = matchup.flip();
        }

        let mut c1 = matchup.god_1().get_num_workers();
        if rng.random_bool(0.1) {
            c1 -= 1;
        };

        let mut c2 = matchup.god_2().get_num_workers();
        if rng.random_bool(0.1) {
            c2 -= 1;
        };

        let mut root_state = get_board_with_random_placements_worker_counters(&mut rng, c1, c2);
        root_state.set_matchup(&matchup);

        if root_state.validation_err().is_err() {
            // eprintln!("Invalid Matchup: {:?}", root_state);
            continue;
        }

        run_match(root_state, &mut rng);
    }
}

// cargo run -p santorini_core --bin fuzzer -r
// cargo run -p santorini_core --bin fuzzer -r -- -g bia -G aphrodite
