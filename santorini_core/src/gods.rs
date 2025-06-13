use super::search::Hueristic;
use crate::{
    board::{Coord, Player, SantoriniState},
    search::judge_state,
};
use mortal::get_mortal_god;
use serde::{Deserialize, Serialize};

pub mod mortal;

pub trait ResultsMapper<T>: Clone {
    fn new() -> Self;
    fn add_action(&mut self, partial_action: PartialAction);
    fn map_result(&self, state: SantoriniState) -> T;
}

// pub type StateWithScore = (SantoriniState, Hueristic);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum PartialAction {
    PlaceWorker(Coord),
    SelectWorker(Coord),
    MoveWorker(Coord),
    Build(Coord),
    NoMoves,
}
type FullAction = Vec<PartialAction>;

#[derive(Clone)]
pub struct FullChoice {
    pub actions: FullAction,
    pub result_state: SantoriniState,
}

impl FullChoice {
    pub fn new(result_state: SantoriniState, action: FullAction) -> Self {
        FullChoice {
            actions: action,
            result_state,
        }
    }
}

pub type PlayerAdvantageFn = fn(&SantoriniState, Player) -> Hueristic;
pub type GenericNextStatesFn<T> = fn(&SantoriniState, Player) -> Vec<T>;
// pub type NextStateWithScoresFn = GenericNextStatesFn<StateWithScore>;
pub type NextStatesOnlyFn = GenericNextStatesFn<SantoriniState>;
pub type NextStatesInteractiveFn = GenericNextStatesFn<FullChoice>;

#[derive(Clone, Debug)]
pub struct StateOnlyMapper {}
impl ResultsMapper<SantoriniState> for StateOnlyMapper {
    fn new() -> Self {
        StateOnlyMapper {}
    }

    fn add_action(&mut self, _partial_action: PartialAction) {}

    fn map_result(&self, state: SantoriniState) -> SantoriniState {
        state
    }
}

/*
#[derive(Clone, Debug)]
pub struct HueristicMapper {}
impl ResultsMapper<StateWithScore> for HueristicMapper {
    fn new() -> Self {
        HueristicMapper {}
    }

    fn add_action(&mut self, _partial_action: PartialAction) {}

    fn map_result(&self, state: SantoriniState) -> StateWithScore {
        let judge_result = judge_state(&state, 0);
        (state, judge_result)
    }
}
*/

#[derive(Clone, Debug)]
pub struct FullChoiceMapper {
    partial_actions: Vec<PartialAction>,
}
impl ResultsMapper<FullChoice> for FullChoiceMapper {
    fn new() -> Self {
        FullChoiceMapper {
            partial_actions: Vec::new(),
        }
    }

    fn add_action(&mut self, partial_action: PartialAction) {
        self.partial_actions.push(partial_action);
    }

    fn map_result(&self, state: SantoriniState) -> FullChoice {
        FullChoice::new(state, self.partial_actions.clone())
    }
}

pub struct GodPower {
    pub player_advantage_fn: PlayerAdvantageFn,
    pub next_states: NextStatesOnlyFn,
    pub next_states_interactive: NextStatesInteractiveFn,
}

pub const ALL_GODS_BY_ID: [GodPower; 1] = [get_mortal_god()];
