use super::search::Hueristic;
use crate::{
    board::{BoardState, FullGameState},
    gods::generic::{GenericMove, ScoredMove},
    player::Player,
    square::Square,
};
// use artemis::build_artemis;
// use hephaestus::build_hephaestus;
// use mortal::build_mortal;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

pub mod generic;
pub mod mortal;

pub type StaticGod = &'static GodPower;

// pub mod artemis;
// pub mod hephaestus;
// pub mod pan;

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum GodName {
    Mortal = 0,
    Pan = 1,
    Artemis = 2,
    Hephaestus = 3,
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
    pub get_all_moves: fn(board: &BoardState, player: Player) -> Vec<ScoredMove>,
    pub get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,
    _get_wins: fn(board: &BoardState, player: Player) -> Vec<ScoredMove>,
    _score_improvers: fn(board: &BoardState, move_list: &mut [ScoredMove]),
    _score_remaining: fn(board: &BoardState, move_list: &mut [ScoredMove]),
    _get_moves: fn(board: &BoardState, player: Player) -> Vec<ScoredMove>,
    _get_moves_without_scores: fn(board: &BoardState, player: Player) -> Vec<ScoredMove>,
    _make_move: fn(board: &mut BoardState, action: GenericMove),
    _unmake_move: fn(board: &mut BoardState, action: GenericMove),
}

impl GodPower {
    pub fn get_next_states_interactive(&self, board: &BoardState) -> Vec<BoardStateWithAction> {
        let all_moves = (self.get_all_moves)(board, board.current_player);

        // Lose due to no moves
        if all_moves.len() == 0 {
            let mut losing_board = board.clone();
            losing_board.set_winner(!board.current_player);

            return vec![BoardStateWithAction::new(
                losing_board,
                vec![PartialAction::NoMoves],
            )];
        }

        all_moves
            .into_iter()
            .flat_map(|action| {
                let mut result_state = board.clone();
                self.make_move(&mut result_state, action.action);
                let action_paths = (self.get_actions_for_move)(board, action.action);

                action_paths.into_iter().map(move |full_actions| {
                    BoardStateWithAction::new(result_state.clone(), full_actions)
                })
            })
            .collect()
    }

    pub fn get_all_next_states(&self, board: &BoardState) -> Vec<BoardState> {
        (self.get_all_moves)(board, board.current_player)
            .into_iter()
            .map(|action| {
                let mut result_state = board.clone();
                self.make_move(&mut result_state, action.action);
                result_state
            })
            .collect()
    }

    pub fn get_moves_for_search(&self, board: &BoardState, player: Player) -> Vec<ScoredMove> {
        (self._get_moves)(board, player)
    }

    pub fn get_moves_for_quiessence(&self, board: &BoardState, player: Player) -> Vec<ScoredMove> {
        (self._get_moves_without_scores)(board, player)
    }

    pub fn make_move(&self, board: &mut BoardState, action: GenericMove) {
        (self._make_move)(board, action);
        board.flip_current_player();
    }

    pub fn unmake_move(&self, board: &mut BoardState, action: GenericMove) {
        board.flip_current_player();
        (self._unmake_move)(board, action);
    }

    pub fn score_improvers(&self, board: &BoardState, move_list: &mut [ScoredMove]) {
        (self._score_improvers)(board, move_list);
    }

    pub fn score_remaining(&self, board: &BoardState, move_list: &mut [ScoredMove]) {
        (self._score_remaining)(board, move_list);
    }

    pub fn get_winning_moves(&self, board: &BoardState, player: Player) -> Vec<ScoredMove> {
        (self._get_wins)(board, player)
    }
}

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
    mortal::build_mortal(),
    // pan::build_pan(),
    // artemis::build_artemis(),
    // hephaestus::build_hephaestus(),
];

#[cfg(test)]
mod tests {
    use super::*;

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
