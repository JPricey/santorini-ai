use super::search::Hueristic;
use crate::board::{BoardState, Coord, FullGameState, Player};
use artemis::build_artemis;
use mortal::build_mortal;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

pub mod artemis;
pub mod mortal;

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum GodName {
    Mortal = 0,
    Artemis = 1,
}

impl GodName {
    pub fn to_power(&self) -> &'static GodPower {
        &ALL_GODS_BY_ID[*self as usize]
    }
}

pub trait ResultsMapper<T>: Clone {
    fn new() -> Self;
    fn add_action(&mut self, partial_action: PartialAction);
    fn map_result(&self, state: BoardState) -> T;
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
pub struct BoardStateWithAction {
    pub result_state: BoardState,
    pub actions: FullAction,
}

impl BoardStateWithAction {
    pub fn new(result_state: BoardState, action: FullAction) -> Self {
        BoardStateWithAction {
            actions: action,
            result_state,
        }
    }
}

#[derive(Clone)]
pub struct GameStateWithAction {
    pub state: FullGameState,
    pub actions: FullAction,
}

impl GameStateWithAction {
    pub fn new(board_state_with_action: BoardStateWithAction, p1: GodName, p2: GodName) -> Self {
        GameStateWithAction {
            state: FullGameState {
                board: board_state_with_action.result_state,
                p1_god: p1.to_power(),
                p2_god: p2.to_power(),
            },
            actions: board_state_with_action.actions,
        }
    }
}

pub type PlayerAdvantageFn = fn(&BoardState, Player) -> Hueristic;
pub type GenericNextStatesFn<T> = fn(&BoardState, Player) -> Vec<T>;
// pub type NextStateWithScoresFn = GenericNextStatesFn<StateWithScore>;
pub type NextStatesOnlyFn = GenericNextStatesFn<BoardState>;
pub type NextStatesInteractiveFn = GenericNextStatesFn<BoardStateWithAction>;

#[derive(Clone, Debug)]
pub struct StateOnlyMapper {}
impl ResultsMapper<BoardState> for StateOnlyMapper {
    fn new() -> Self {
        StateOnlyMapper {}
    }

    fn add_action(&mut self, _partial_action: PartialAction) {}

    fn map_result(&self, state: BoardState) -> BoardState {
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
impl ResultsMapper<BoardStateWithAction> for FullChoiceMapper {
    fn new() -> Self {
        FullChoiceMapper {
            partial_actions: Vec::new(),
        }
    }

    fn add_action(&mut self, partial_action: PartialAction) {
        self.partial_actions.push(partial_action);
    }

    fn map_result(&self, state: BoardState) -> BoardStateWithAction {
        BoardStateWithAction::new(state, self.partial_actions.clone())
    }
}

pub struct GodPower {
    pub god_name: GodName,
    pub player_advantage_fn: PlayerAdvantageFn,
    pub next_states: NextStatesOnlyFn,
    pub next_states_interactive: NextStatesInteractiveFn,
}

impl PartialEq for GodPower {
    fn eq(&self, other: &Self) -> bool {
        self.god_name == other.god_name
    }
}

impl Eq for GodPower {}

pub const ALL_GODS_BY_ID: [GodPower; 2] = [build_mortal(), build_artemis()];
