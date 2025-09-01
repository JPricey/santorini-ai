use rand::{
    Rng, rng,
    seq::{IndexedRandom, IteratorRandom},
};

use crate::{
    board::FullGameState,
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
};

pub fn get_board_with_random_placements_worker_counters(
    rng: &mut impl Rng,
    c1: usize,
    c2: usize,
) -> FullGameState {
    let mut result = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
    let worker_spots = (0..25).choose_multiple(rng, c1 + c2);
    let mut iter = worker_spots.iter();

    for _ in 0..c1 {
        result.board.workers[0].0 |= 1 << iter.next().unwrap();
    }
    for _ in 0..c2 {
        result.board.workers[1].0 |= 1 << iter.next().unwrap();
    }

    result
}

pub fn get_board_with_random_placements(rng: &mut impl Rng) -> FullGameState {
    get_board_with_random_placements_worker_counters(rng, 2, 2)
}

pub fn get_random_god(rng: &mut impl Rng) -> StaticGod {
    ALL_GODS_BY_ID.choose(rng).unwrap()
}

pub fn get_random_move(state: &FullGameState, rng: &mut impl Rng) -> Option<FullGameState> {
    if state.board.get_winner().is_some() {
        return None;
    }
    let child_states = state.get_next_states();
    child_states.iter().choose(rng).cloned()
}

pub struct RandomSingleGameStateGenerator {
    current_state: Option<FullGameState>,
}

impl RandomSingleGameStateGenerator {
    pub fn new(mut initial_state: FullGameState) -> Self {
        initial_state.recalculate_internals();
        RandomSingleGameStateGenerator {
            current_state: Some(initial_state),
        }
    }

    pub fn peek_unsafe(&self) -> FullGameState {
        self.current_state.clone().unwrap()
    }
}

impl Default for RandomSingleGameStateGenerator {
    fn default() -> Self {
        Self::new(get_board_with_random_placements(&mut rng()))
    }
}

impl Iterator for RandomSingleGameStateGenerator {
    type Item = FullGameState;

    fn next(&mut self) -> Option<FullGameState> {
        match self.current_state.take() {
            None => None,
            Some(result) => {
                self.current_state = get_random_move(&result, &mut rng());
                Some(result)
            }
        }
    }
}

pub struct GameStateFuzzer {
    current_generator: RandomSingleGameStateGenerator,
    remaining_states: usize,
}

impl GameStateFuzzer {
    pub fn new(num_states: usize) -> Self {
        GameStateFuzzer {
            current_generator: RandomSingleGameStateGenerator::default(),
            remaining_states: num_states,
        }
    }
}

impl Default for GameStateFuzzer {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl Iterator for GameStateFuzzer {
    type Item = FullGameState;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_states == 0 {
            return None;
        }
        self.remaining_states -= 1;

        for _ in 0..10 {
            match self.current_generator.next() {
                None => self.current_generator = RandomSingleGameStateGenerator::default(),
                Some(result) => return Some(result),
            }
        }
        panic!("Couldn't generate new random positions");
    }
}
