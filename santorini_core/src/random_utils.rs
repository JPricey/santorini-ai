use rand::{
    Rng,
    seq::{IteratorRandom, SliceRandom},
    thread_rng,
};

use crate::{board::FullGameState, gods::GodName};

pub fn get_board_with_random_placements(rng: &mut impl Rng) -> FullGameState {
    let mut result = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
    let worker_spots: Vec<usize> = (0..25).choose_multiple(rng, 4).iter().cloned().collect();

    result.board.workers[0].0 |= 1 << worker_spots[0];
    result.board.workers[0].0 |= 1 << worker_spots[1];

    result.board.workers[1].0 |= 1 << worker_spots[2];
    result.board.workers[1].0 |= 1 << worker_spots[3];

    result
}

pub fn get_random_move(state: &FullGameState, rng: &mut impl Rng) -> Option<FullGameState> {
    if state.board.get_winner().is_some() {
        return None;
    }
    let child_states = state.get_next_states();
    child_states.choose(rng).cloned()
}

pub struct RandomSingleGameStateGenerator {
    current_state: Option<FullGameState>,
}

impl RandomSingleGameStateGenerator {
    pub fn new(initial_state: FullGameState) -> Self {
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
        Self::new(get_board_with_random_placements(&mut thread_rng()))
    }
}

impl Iterator for RandomSingleGameStateGenerator {
    type Item = FullGameState;

    fn next(&mut self) -> Option<FullGameState> {
        match self.current_state.take() {
            None => None,
            Some(result) => {
                self.current_state = get_random_move(&result, &mut thread_rng());
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
