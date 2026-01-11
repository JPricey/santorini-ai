use rand::{
    Rng, rng,
    seq::{IndexedRandom, IteratorRandom},
};

use crate::{
    board::FullGameState,
    gods::{
        ALL_GODS_BY_ID, GodName, StaticGod, generic::ScoredMove, jason::JasonMove,
        polyphemus::PolyphemusMove,
    },
    matchup::Matchup,
    placement::get_starting_placement_state,
};

pub fn get_random_starting_state<T: Rng>(matchup: &Matchup, rng: &mut T) -> FullGameState {
    let mut state = FullGameState::new_for_matchup(matchup);

    for _ in 0..2 {
        let placement_state = get_starting_placement_state(&state.board, state.gods)
            .unwrap()
            .unwrap();
        let active_player = placement_state.next_placement;
        let (active_god, other_god) = state.get_player_non_player_gods(active_player);

        let placement_actions =
            active_god.get_all_placement_actions(state.gods, &state.board, active_player);
        let action = placement_actions.choose(rng).unwrap().clone();

        active_god.make_placement_move(action, &mut state.board, active_player, other_god);
    }

    state
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

fn choose_move_flattening_power(
    state: &FullGameState,
    all_moves: &[ScoredMove],
    rng: &mut impl Rng,
    is_power_move: impl Fn(&ScoredMove) -> bool,
) -> Option<ScoredMove> {
    let has_power = state.board.god_data[state.board.current_player as usize] == 0;

    if has_power && rng.random_bool(0.95) {
        if let Some(no_power_move) = all_moves.iter().filter(|a| !is_power_move(a)).choose(rng) {
            return Some(no_power_move.clone());
        }
    }
    all_moves.iter().choose(rng).cloned()
}

pub fn get_random_move_flattening_powers(
    state: &FullGameState,
    rng: &mut impl Rng,
) -> Option<ScoredMove> {
    if state.board.get_winner().is_some() {
        return None;
    }
    let active_god = state.get_active_god();
    let all_moves = active_god.get_all_moves(state, state.board.current_player);

    match active_god.god_name {
        GodName::Polyphemus => choose_move_flattening_power(state, &all_moves, rng, |a| {
            let poly_move: PolyphemusMove = a.action.into();
            poly_move.dome_1().is_some()
        }),
        GodName::Jason => choose_move_flattening_power(state, &all_moves, rng, |a| {
            let jason_move: JasonMove = a.action.into();
            jason_move.maybe_place_position().is_some()
        }),
        _ => all_moves.iter().choose(rng).cloned(),
    }
}

pub fn get_random_state_flattening_powers(
    state: &FullGameState,
    rng: &mut impl Rng,
) -> Option<FullGameState> {
    let scored_move = get_random_move_flattening_powers(state, rng);
    if let Some(scored_move) = scored_move {
        let (active_god, other_god) = state.get_active_non_active_gods();
        Some(state.next_state(active_god, other_god, scored_move.action))
    } else {
        None
    }
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
        Self::new(get_random_starting_state(
            &Matchup::new(GodName::Mortal, GodName::Mortal),
            &mut rng(),
        ))
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
        Self::new(1_000)
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
