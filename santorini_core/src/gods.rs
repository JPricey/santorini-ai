use super::search::Hueristic;
use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState},
    gods::generic::{GenericMove, GodMove, ScoredMove},
    hashing::HashType,
    player::Player,
    square::Square,
    utils::hash_u64,
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
pub mod urania;
pub mod graeae;

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
    Urania = 11,
    Graeae = 12,
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

pub type FullAction = Vec<PartialAction>;

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

pub struct GodPowerMoveFns {
    _get_all_moves:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_wins: fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_win_blockers:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_moves_for_search:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
}

pub struct GodPowerActionFns {
    _get_blocker_board: fn(board: &BoardState, action: GenericMove) -> BitBoard,
    _get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,

    _make_move: fn(board: &mut BoardState, action: GenericMove),
    _unmake_move: fn(board: &mut BoardState, action: GenericMove),

    _get_history_hash: fn(board: &BoardState, action: GenericMove) -> usize,
    _stringify_move: fn(action: GenericMove) -> String,
}

pub struct GodPower {
    pub god_name: GodName,
    pub model_god_name: GodName,

    // Move Fns
    pub _get_all_moves:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_wins: fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_win_blockers:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,
    _get_moves_for_search:
        fn(board: &BoardState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>,

    // Action Fns
    _get_blocker_board: fn(board: &BoardState, action: GenericMove) -> BitBoard,
    _get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,

    _make_move: fn(board: &mut BoardState, action: GenericMove),
    _unmake_move: fn(board: &mut BoardState, action: GenericMove),

    _get_history_hash: fn(board: &BoardState, action: GenericMove) -> usize,
    _stringify_move: fn(action: GenericMove) -> String,

    // _modify_moves: fn(board: &BoardState, from: Square, to_mask: BitBoard, is_win: bool, is_future: bool),
    pub hash1: HashType,
    pub hash2: HashType,
    // UI
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
                let action_paths = (self._get_actions_for_move)(board, action.action);

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

    pub fn get_blocker_board(&self, board: &BoardState, action: GenericMove) -> BitBoard {
        (self._get_blocker_board)(board, action)
    }

    pub fn make_move(&self, board: &mut BoardState, action: GenericMove) {
        (self._make_move)(board, action);
        board.flip_current_player();
    }

    pub fn unmake_move(&self, board: &mut BoardState, action: GenericMove) {
        board.flip_current_player();
        (self._unmake_move)(board, action);
    }

    pub fn stringify_move(&self, action: GenericMove) -> String {
        (self._stringify_move)(action)
    }

    pub fn get_actions_for_move(&self, board: &BoardState, action: GenericMove) -> Vec<FullAction> {
        (self._get_actions_for_move)(board, action)
    }

    pub fn get_history_hash(&self, board: &BoardState, action: GenericMove) -> usize {
        (self._get_history_hash)(board, action)
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

pub const ALL_GODS_BY_ID: [GodPower; 13] = [
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
    urania::build_urania(),
    graeae::build_graeae(),
];

#[macro_export]
macro_rules! build_god_power_movers {
    (
        $move_gen:ident
    ) => {{
        {
            crate::gods::GodPowerMoveFns {
                _get_all_moves: $move_gen::<0>,
                _get_moves_for_search: $move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
                _get_wins: $move_gen::<{ MATE_ONLY }>,
                _get_win_blockers: $move_gen::<
                    { STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES | INCLUDE_SCORE },
                >,
            }
        }
    }};
}

pub const fn build_god_power_actions<T: GodMove>() -> GodPowerActionFns {
    fn _stringify_move<T: GodMove>(action: GenericMove) -> String {
        let action: T = action.into();
        format!("{:?}", action)
    }

    fn _get_actions_for_move<T: GodMove>(
        board: &BoardState,
        action: GenericMove,
    ) -> Vec<FullAction> {
        let action: T = action.into();
        action.move_to_actions(board)
    }

    fn _make_move<T: GodMove>(board: &mut BoardState, action: GenericMove) {
        let action: T = action.into();
        action.make_move(board)
    }

    fn _unmake_move<T: GodMove>(board: &mut BoardState, action: GenericMove) {
        let action: T = action.into();
        action.unmake_move(board)
    }

    fn _get_blocker_board<T: GodMove>(board: &BoardState, action: GenericMove) -> BitBoard {
        let action: T = action.into();
        action.get_blocker_board(board)
    }

    fn _get_history_hash<T: GodMove>(board: &BoardState, action: GenericMove) -> usize {
        let action: T = action.into();
        hash_u64(action.get_history_idx(&board))
    }

    GodPowerActionFns {
        _get_actions_for_move: _get_actions_for_move::<T>,
        _get_blocker_board: _get_blocker_board::<T>,
        _make_move: _make_move::<T>,
        _unmake_move: _unmake_move::<T>,
        _stringify_move: _stringify_move::<T>,
        _get_history_hash: _get_history_hash::<T>,
    }
}

const fn god_power(
    name: GodName,
    movers: GodPowerMoveFns,
    actions: GodPowerActionFns,
    hash1: u64,
    hash2: u64,
) -> GodPower {
    GodPower {
        god_name: name,
        model_god_name: name,
        _get_all_moves: movers._get_all_moves,
        _get_moves_for_search: movers._get_moves_for_search,
        _get_wins: movers._get_wins,
        _get_win_blockers: movers._get_win_blockers,

        _get_blocker_board: actions._get_blocker_board,
        _get_actions_for_move: actions._get_actions_for_move,
        _make_move: actions._make_move,
        _unmake_move: actions._unmake_move,
        _stringify_move: actions._stringify_move,
        _get_history_hash: actions._get_history_hash,

        hash1,
        hash2,
    }
}

#[macro_export]
macro_rules! build_god_power {
    (
        $fn_name:ident,
        god_name: $god_name:expr,
        move_type: $move_type:ident,
        move_gen: $move_gen:ident,
        hash1: $hash1:expr,
        hash2: $hash2:expr,
    ) => {
        pub const fn $fn_name() -> GodPower {
            use crate::gods::FullAction;
            use crate::gods::generic::GenericMove;
            use crate::gods::generic::GodMove;
            use crate::nnue::NNUE_GOD_COUNT;
            use crate::utils::hash_u64;

            fn _stringify_move(action: GenericMove) -> String {
                let action: $move_type = action.into();
                format!("{:?}", action)
            }

            fn _get_actions_for_move(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
                let action: $move_type = action.into();
                action.move_to_actions(board)
            }

            fn _make_move(board: &mut BoardState, action: GenericMove) {
                let action: $move_type = action.into();
                action.make_move(board)
            }

            fn _unmake_move(board: &mut BoardState, action: GenericMove) {
                let action: $move_type = action.into();
                action.unmake_move(board)
            }

            fn _get_blocker_board(board: &BoardState, action: GenericMove) -> BitBoard {
                let action: $move_type = action.into();
                action.get_blocker_board(board)
            }

            fn _get_history_hash(board: &BoardState, action: GenericMove) -> usize {
                let action: $move_type = action.into();
                hash_u64(action.get_history_idx(&board))
            }

            let model_god_name = if ($god_name) as usize >= NNUE_GOD_COUNT {
                GodName::Mortal
            } else {
                $god_name
            };

            GodPower {
                god_name: $god_name,
                model_god_name: model_god_name,
                _get_all_moves: $move_gen::<0>,
                _get_moves_for_search: $move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
                _get_wins: $move_gen::<{ MATE_ONLY }>,
                _get_win_blockers: $move_gen::<
                    { STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES | INCLUDE_SCORE },
                >,

                _get_actions_for_move,
                _get_blocker_board,
                _make_move,
                _unmake_move,
                _stringify_move,
                _get_history_hash,

                hash1: $hash1,
                hash2: $hash2,
            }
        }
    };
}

impl GodPower {
    pub const fn with_nnue_god_name(mut self, name: GodName) -> Self {
        self.model_god_name = name;
        self
    }
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
