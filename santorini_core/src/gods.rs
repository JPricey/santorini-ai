use super::search::Hueristic;
use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState},
    gods::generic::{GenericMove, ScoredMove},
    hashing::HashType,
    player::Player,
    square::Square,
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

pub mod apollo;
pub mod artemis;
pub mod athena;
pub mod atlas;
pub mod demeter;
pub mod generic;
pub mod hephaestus;
pub mod hermes;
pub mod minotaur;
pub mod mortal;
pub mod pan;
pub mod prometheus;

pub type StaticGod = &'static GodPower;

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Debug,
    Display,
    Serialize,
    Deserialize,
    EnumString,
    IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum GodName {
    Mortal = 0,
    Pan = 1,
    Artemis = 2,
    Hephaestus = 3,
    Atlas = 4,
    Athena = 5,
    Minotaur = 6,
    Demeter = 7,
    Apollo = 8,
    Hermes = 9,
    Prometheus = 10,
}

impl GodName {
    pub fn to_power(&self) -> StaticGod {
        &ALL_GODS_BY_ID[*self as usize]
    }
}

pub trait ResultsMapper<T>: Clone {
    fn new() -> Self;
    fn add_action(&mut self, partial_action: PartialAction);
    fn map_result(&self, state: BoardState) -> T;
}

pub type StateWithScore = (BoardState, Hueristic);

/*
pub enum MoveWorkerMeta {
    None,
    IsSwap,
    Push(Square),
}
*/

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum PartialAction {
    PlaceWorker(Square),
    SelectWorker(Square),
    MoveWorker(Square),
    MoveWorkerWithSwap(Square),
    MoveWorkerWithPush(Square, Square),
    Build(Square),
    Dome(Square),
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

    // Move Generators
    pub _get_all_moves:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_wins: fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_win_blockers:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_moves_for_search:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,

    // Move Scorers
    _score_improvers: fn(board: &BoardState, move_list: &mut [ScoredMove]),
    _score_remaining: fn(board: &BoardState, move_list: &mut [ScoredMove]),

    // Check detection
    _get_blocker_board: fn(action: GenericMove) -> BitBoard,

    // Make/Unmake
    _make_move: fn(board: &mut BoardState, action: GenericMove),
    _unmake_move: fn(board: &mut BoardState, action: GenericMove),

    _stringify_move: fn(action: GenericMove) -> String,

    pub hash1: HashType,
    pub hash2: HashType,
    // UI
    pub get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,
}

impl GodPower {
    pub fn get_next_states_interactive(&self, board: &BoardState) -> Vec<BoardStateWithAction> {
        let all_moves = (self._get_all_moves)(board, board.current_player, BitBoard::EMPTY);

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
        (self._get_all_moves)(board, board.current_player, BitBoard::EMPTY)
            .into_iter()
            .map(|action| {
                let mut result_state = board.clone();
                self.make_move(&mut result_state, action.action);
                result_state
            })
            .collect()
    }

    pub fn get_moves_for_search(&self, board: &BoardState, player: Player) -> Vec<ScoredMove> {
        (self._get_moves_for_search)(board, player, BitBoard::EMPTY)
    }

    pub fn get_winning_moves(&self, board: &BoardState, player: Player) -> Vec<ScoredMove> {
        (self._get_wins)(board, player, BitBoard::EMPTY)
    }

    pub fn get_blocker_moves(
        &self,
        board: &BoardState,
        player: Player,
        key_moves: BitBoard,
    ) -> Vec<ScoredMove> {
        (self._get_win_blockers)(board, player, key_moves)
    }

    pub fn get_blocker_board(&self, action: GenericMove) -> BitBoard {
        (self._get_blocker_board)(action)
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

    pub fn stringify_move(&self, action: GenericMove) -> String {
        (self._stringify_move)(action)
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

impl std::fmt::Display for GodPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.god_name)
    }
}

pub const ALL_GODS_BY_ID: [GodPower; 11] = [
    mortal::build_mortal(),
    pan::build_pan(),
    artemis::build_artemis(),
    hephaestus::build_hephaestus(),
    atlas::build_atlas(),
    athena::build_athena(),
    minotaur::build_minotaur(),
    demeter::build_demeter(),
    apollo::build_apollo(),
    hermes::build_hermes(),
    prometheus::build_prometheus(),
];

#[macro_export]
macro_rules! build_god_power {
    (
        $fn_name:ident,
        god_name: $god_name:expr,
        move_gen: $move_gen:ident,
        actions: $actions_fn:ident,
        score_moves: $score_moves:ident,
        blocker_board: $blocker_board_fn:ident,
        make_move: $make_move_fn:ident,
        unmake_move: $unmake_move_fn:ident,
        stringify: $stringify_fn:ident,
        hash1: $hash1:expr,
        hash2: $hash2:expr,
    ) => {
        pub const fn $fn_name() -> GodPower {
            GodPower {
                god_name: $god_name,
                _get_all_moves: $move_gen::<0>,
                _get_moves_for_search: $move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
                _get_wins: $move_gen::<{ MATE_ONLY }>,
                _get_win_blockers: $move_gen::<
                    { STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES | INCLUDE_SCORE },
                >,
                get_actions_for_move: $actions_fn,
                _score_improvers: $score_moves::<true>,
                _score_remaining: $score_moves::<false>,
                _get_blocker_board: $blocker_board_fn,
                _make_move: $make_move_fn,
                _unmake_move: $unmake_move_fn,
                _stringify_move: $stringify_fn,

                hash1: $hash1,
                hash2: $hash2,
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{Rng, rng};

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

    #[test]
    fn test_all_hashes_are_unique() {
        let mut rng = rng();
        let suggestion: HashType = rng.random_range(0..u64::MAX);
        let mut all_hashes: HashSet<HashType> = HashSet::new();
        for god_power in ALL_GODS_BY_ID.iter() {
            assert!(
                !all_hashes.contains(&god_power.hash1),
                "hash1 number {} for {:?} is not unique. Here's a new suggestion: {}",
                god_power.hash1,
                god_power.god_name,
                suggestion
            );
            all_hashes.insert(god_power.hash1);

            assert!(
                !all_hashes.contains(&god_power.hash2),
                "hash2 number {} for {:?} is not unique. Here's a new suggestion: {}",
                god_power.hash2,
                god_power.god_name,
                suggestion
            );
            all_hashes.insert(god_power.hash2);
        }
    }
}
