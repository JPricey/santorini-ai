use super::search::Hueristic;
use crate::{
    board::{BoardState, FullGameState},
    gods::generic::{ANY_MATE_CHECK, STOP_ON_MATE, mortal_move_gen},
    move_container::GenericMove,
    player::Player,
    square::Square,
};
// use artemis::build_artemis;
// use hephaestus::build_hephaestus;
// use mortal::build_mortal;
// use pan::build_pan;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

pub mod generic;
// pub mod mortal;

// pub mod artemis;
// pub mod hephaestus;
// pub mod pan;

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum GodName {
    Mortal = 0,
    // Artemis = 1,
    // Hephaestus = 2,
    // Pan = 3,
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

pub type StateWithScore = (BoardState, Hueristic);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum PartialAction {
    PlaceWorker(Square),
    SelectWorker(Square),
    MoveWorker(Square),
    Build(Square),
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
                gods: [p1.to_power(), p2.to_power()],
            },
            actions: board_state_with_action.actions,
        }
    }
}

pub type PlayerAdvantageFn = fn(&BoardState, Player) -> Hueristic;
pub type GenericNextStatesFn<T> = fn(&BoardState, Player) -> Vec<T>;
pub type NextStateWithScoresFn = GenericNextStatesFn<StateWithScore>;
pub type NextStatesOnlyFn = GenericNextStatesFn<BoardState>;
pub type NextStatesInteractiveFn = GenericNextStatesFn<BoardStateWithAction>;
pub type HasWinFn = fn(&BoardState, Player) -> bool;

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
    pub get_all_moves: fn(board: &BoardState, player: Player) -> Vec<GenericMove>,
    pub get_actions_for_move:
        fn(board: &BoardState, action: GenericMove) -> Vec<BoardStateWithAction>,
    pub get_moves: fn(board: &BoardState, player: Player) -> Vec<GenericMove>,
    pub get_wins: fn(board: &BoardState, player: Player) -> Vec<GenericMove>,
    pub make_move: fn(board: &mut BoardState, action: GenericMove),
    pub unmake_move: fn(board: &mut BoardState, action: GenericMove),
}

impl GodPower {
    pub fn get_next_states(&self, board: &BoardState) -> Vec<BoardStateWithAction> {
        (self.get_all_moves)(board, board.current_player)
            .into_iter()
            .flat_map(|action| {
                let mut new_board = board.clone();
                let result_state = (self.make_move)(&mut new_board, action);
                let action_paths = (self.get_actions_for_move)(board, action);

                action_paths
                    .into_iter()
                    .map(|full_actions| BoardStateWithAction::new(new_board.clone(), full_actions))
            })
            .collect()
    }
}

/*
pub struct GodPower {
    pub god_name: GodName,
    pub player_advantage_fn: PlayerAdvantageFn,
    pub next_states: NextStatesOnlyFn,
    pub next_states_interactive: NextStatesInteractiveFn,
    pub has_win: HasWinFn,
}
*/

impl std::fmt::Debug for GodPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GodPower({:?})", self.god_name)
    }
}

impl PartialEq for GodPower {
    fn eq(&self, other: &Self) -> bool {
        self.god_name == other.god_name
    }
}

impl Eq for GodPower {}

pub const ALL_GODS_BY_ID: [GodPower; 1] = [
    GodPower {
        god_name: GodName::Mortal,
        get_all_moves: mortal_move_gen::<0>,
        get_moves: mortal_move_gen::<{ STOP_ON_MATE }>,
        get_wins: mortal_move_gen::<{ ANY_MATE_CHECK }>,
    },
    // build_mortal(),
    // build_artemis(),
    // build_hephaestus(),
    // build_pan(),
];

#[cfg(test)]
mod tests {
    use crate::board::FullGameState;

    use super::*;

    fn _slow_win_check(state: &FullGameState) -> bool {
        let child_state = state.get_next_states();
        for child in child_state {
            if child.board.get_winner() == Some(state.board.current_player) {
                return true;
            }
        }
        return false;
    }

    pub fn assert_has_win_consistency(state: &FullGameState, expected_has_win: bool) {
        let slow_win_check_result = _slow_win_check(state);
        assert_eq!(
            slow_win_check_result, expected_has_win,
            "State was meant to have win expectation: {:?}, but was {:?}: {:?}",
            expected_has_win, slow_win_check_result, state
        );

        let fast_win_check =
            (state.get_active_god().has_win)(&state.board, state.board.current_player);
        assert_eq!(
            fast_win_check, expected_has_win,
            "State has_win was meant to have win expectation: {:?}, but was {:?}: {:?}",
            expected_has_win, slow_win_check_result, state
        );
    }

    #[test]
    fn test_god_alignment() {
        for (i, god_power) in ALL_GODS_BY_ID.iter().enumerate() {
            assert_eq!(
                god_power.god_name as usize, i,
                "God {:?} is in the wrong position",
                god_power.god_name
            );
        }
    }
}
