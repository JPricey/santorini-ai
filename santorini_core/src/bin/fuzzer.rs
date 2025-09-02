use rand::{Rng, rng, seq::IndexedRandom};

use santorini_core::{
    board::FullGameState,
    consistency_checker::consistency_check,
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
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
            // current_state.print_to_console();
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

fn main() {
    let mut rng = rng();

    let god1_selector = GodRandomizer::new_any();
    // let god1_selector = GodRandomizer::new_exactly(GodName::Hypnus);

    let god2_selector = GodRandomizer::new_any();
    // let god2_selector = GodRandomizer::new_exactly(GodName::Artemis);
    // let god2_selector = GodRandomizer::new_not_one_of(&vec![GodName::Artemis]);

    loop {
        let mut g1 = god1_selector.get();
        let mut g2 = god2_selector.get();

        if rng.random_bool(0.5) {
            std::mem::swap(&mut g1, &mut g2);
        }

        let mut c1 = g1.num_workers;
        if rng.random_bool(0.1) {
            c1 -= 1;
        };

        let mut c2 = g2.num_workers;
        if rng.random_bool(0.1) {
            c2 -= 1;
        };

        let mut root_state = get_board_with_random_placements_worker_counters(&mut rng, c1, c2);

        root_state.gods[0] = g1;
        root_state.gods[1] = g2;
        root_state.recalculate_internals();

        if root_state.validation_err().is_err() {
            // eprintln!("Invalid Matchup: {:?}", root_state);
            continue;
        }

        run_match(root_state, &mut rng);
    }
}
