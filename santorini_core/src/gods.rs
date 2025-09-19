use super::search::Hueristic;
use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    direction::Direction,
    gods::generic::{GenericMove, GodMove, ScoredMove},
    hashing::HashType,
    player::Player,
    square::Square,
    utils::hash_u64,
};
use counted_array::counted_array;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

pub(crate) mod aeolus;
pub(crate) mod aphrodite;
pub(crate) mod apollo;
pub(crate) mod artemis;
pub(crate) mod athena;
pub(crate) mod atlas;
pub(crate) mod bia;
pub(crate) mod clio;
pub(crate) mod demeter;
pub(crate) mod europa;
pub mod generic;
pub(crate) mod graeae;
pub(crate) mod hades;
pub(crate) mod harpies;
pub(crate) mod hephaestus;
pub(crate) mod hera;
pub(crate) mod hermes;
pub(crate) mod hestia;
pub(crate) mod hypnus;
pub(crate) mod limus;
pub(crate) mod maenads;
pub(crate) mod minotaur;
pub(crate) mod morpheus;
pub(crate) mod mortal;
pub(crate) mod move_helpers;
pub(crate) mod pan;
pub(crate) mod persephone;
pub(crate) mod prometheus;
pub(crate) mod urania;

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
    PartialOrd,
    Ord,
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
    Hera = 13,
    Limus = 14,
    Hypnus = 15,
    Harpies = 16,
    Aphrodite = 17,
    Persephone = 18,
    Hades = 19,
    Morpheus = 20,
    Aeolus = 21,
    Hestia = 22,
    Europa = 23,
    Bia = 24,
    Clio = 25,
    Maenads = 26,
}

// pub const WIP_GODS: [GodName; 0] = [];
counted_array!(pub const WIP_GODS: [GodName; _] = [
    GodName::Aphrodite,
    GodName::Persephone,
    GodName::Hades,
    GodName::Morpheus,
    GodName::Aeolus,
    GodName::Hestia,
    GodName::Europa,
    GodName::Bia,
    GodName::Clio,
    GodName::Maenads,
]);

impl GodName {
    pub const fn to_power(&self) -> StaticGod {
        &ALL_GODS_BY_ID[*self as usize]
    }

    pub const fn is_equal(self, other: GodName) -> bool {
        self as usize == other as usize
    }
}

pub trait ResultsMapper<T>: Clone {
    fn new() -> Self;
    fn add_action(&mut self, partial_action: PartialAction);
    fn map_result(&self, state: BoardState) -> T;
}

pub type StateWithScore = (BoardState, Hueristic);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct MoveEnemyWorker {
    pub from: Square,
    pub to: Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct KillEnemyWorker {
    pub square: Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum MoveWorkerMeta {
    MoveEnemyWorker(MoveEnemyWorker),
    KillEnemyWorker(KillEnemyWorker),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct MoveWorkerData {
    pub dest: Square,
    pub meta: Option<MoveWorkerMeta>,
}

impl From<Square> for MoveWorkerData {
    fn from(value: Square) -> Self {
        MoveWorkerData {
            dest: value,
            meta: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum PartialAction {
    SelectWorker(Square),
    PlaceWorker(Square),
    MoveWorker(MoveWorkerData),
    Build(Square),
    Dome(Square),
    SetTalusPosition(Square),
    SetWindDirection(Option<Direction>),
    NoMoves,
    EndTurn,
}

impl PartialAction {
    fn new_move_with_displace(dest: Square, swap_from: Square, swap_to: Square) -> PartialAction {
        PartialAction::MoveWorker(MoveWorkerData {
            dest,
            meta: Some(MoveWorkerMeta::MoveEnemyWorker(MoveEnemyWorker {
                from: swap_from,
                to: swap_to,
            })),
        })
    }

    fn new_move_with_kill(dest: Square, kill_pos: Square) -> PartialAction {
        PartialAction::MoveWorker(MoveWorkerData {
            dest,
            meta: Some(MoveWorkerMeta::KillEnemyWorker(KillEnemyWorker {
                square: kill_pos,
            })),
        })
    }
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

#[derive(Clone, Debug)]
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

pub(super) type ParseGodDataFn = fn(&str) -> Result<GodData, String>;
pub(super) type StringifyGodDataFn = fn(GodData) -> Option<String>;
fn _default_parse_god_data(fen: &str) -> Result<GodData, String> {
    match fen {
        "" => Ok(0),
        _ => Err(format!("Unknown god data format: {}", fen)),
    }
}

fn _default_stringify_god_data(_data: GodData) -> Option<String> {
    None
}
pub(super) type PrettyStringifyGodDataFn = fn(&BoardState, Player) -> Option<String>;
fn _default_pretty_stringify_god_data(_board: &BoardState, _player: Player) -> Option<String> {
    None
}

pub(super) type GetWindIdxFn = fn(&BoardState, Player) -> usize;
fn _default_get_wind_idx(_board: &BoardState, _player: Player) -> usize {
    0
}

pub(super) type MoveGeneratorFn =
    fn(board: &FullGameState, player: Player, key_squares: BitBoard) -> Vec<ScoredMove>;

pub(super) struct GodPowerMoveFns {
    _get_all_moves: MoveGeneratorFn,
    _get_wins: MoveGeneratorFn,
    _get_unscored_win_blockers: MoveGeneratorFn,
    _get_scored_win_blockers: MoveGeneratorFn,
    _get_moves_for_search: MoveGeneratorFn,
}

pub(super) struct GodPowerActionFns {
    _get_blocker_board: fn(board: &BoardState, action: GenericMove) -> BitBoard,
    _get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,

    _make_move: fn(board: &mut BoardState, action: GenericMove),

    _get_history_hash: fn(board: &BoardState, action: GenericMove) -> usize,
    _stringify_move: fn(action: GenericMove) -> String,
}

pub(super) type BuildMaskFn = fn(oppo_workers: BitBoard) -> BitBoard;
fn _default_build_mask(_oppo_workers: BitBoard) -> BitBoard {
    BitBoard::MAIN_SECTION_MASK
}

pub(super) type MovableWorkerFilter = fn(board: &BoardState, workers: BitBoard) -> BitBoard;
fn _default_moveable_worker_filter(_board: &BoardState, workers: BitBoard) -> BitBoard {
    workers
}

pub(super) type CanOpponentClimbFn = fn(&BoardState, Player) -> bool;
fn _default_can_opponent_climb(_board: &BoardState, _player: Player) -> bool {
    true
}

pub(super) type MakePassingMoveFn = fn(&mut BoardState);
fn _default_passing_move(_board: &mut BoardState) {
    // Noop
}

pub(super) type GetFrozenMask = fn(&BoardState, Player) -> BitBoard;
fn _default_get_frozen_mask(_board: &BoardState, _player: Player) -> BitBoard {
    BitBoard::EMPTY
}

pub(super) type FlipGodDataFn = fn(GodData) -> GodData;
fn _default_flip_god_data(god_data: GodData) -> GodData {
    god_data
}

pub struct GodPower {
    pub god_name: GodName,
    pub model_god_name: GodName,

    // Move Fns
    pub _get_all_moves: MoveGeneratorFn,
    _get_wins: MoveGeneratorFn,
    _get_scored_win_blockers: MoveGeneratorFn,
    _get_unscored_win_blockers: MoveGeneratorFn,
    _get_moves_for_search: MoveGeneratorFn,

    // God specific move blockers
    _build_mask_fn: BuildMaskFn,
    _moveable_worker_filter_fn: MovableWorkerFilter,
    _can_opponent_climb_fn: CanOpponentClimbFn,
    _get_frozen_mask: GetFrozenMask,
    pub win_mask: BitBoard,

    _flip_god_data_horizontal: FlipGodDataFn,
    _flip_god_data_vertical: FlipGodDataFn,
    _flip_god_data_transpose: FlipGodDataFn,

    // Action Fns
    _get_blocker_board: fn(board: &BoardState, action: GenericMove) -> BitBoard,
    _get_actions_for_move: fn(board: &BoardState, action: GenericMove) -> Vec<FullAction>,

    _make_move: fn(board: &mut BoardState, action: GenericMove),
    _make_passing_move: MakePassingMoveFn,

    _get_history_hash: fn(board: &BoardState, action: GenericMove) -> usize,
    _stringify_move: fn(action: GenericMove) -> String,

    _parse_god_data: ParseGodDataFn,
    _stringify_god_data: StringifyGodDataFn,

    _pretty_stringify_god_data: PrettyStringifyGodDataFn,

    pub num_workers: usize,

    pub is_aphrodite: bool,
    pub is_persephone: bool,
    pub is_preventing_down: bool,
    pub is_placement_priority: bool,

    _get_wind_idx: GetWindIdxFn,

    // _modify_moves: fn(board: &BoardState, from: Square, to_mask: BitBoard, is_win: bool, is_future: bool),
    pub hash1: HashType,
    pub hash2: HashType,
    // UI
}

impl GodPower {
    pub fn get_build_mask(&self, own_workers: BitBoard) -> BitBoard {
        (self._build_mask_fn)(own_workers)
    }

    pub fn get_moveable_workers(&self, board: &BoardState, workers: BitBoard) -> BitBoard {
        (self._moveable_worker_filter_fn)(board, workers)
    }

    pub fn is_hypnus(&self) -> bool {
        self.god_name == GodName::Hypnus
    }

    pub fn is_harpies(&self) -> bool {
        self.god_name == GodName::Harpies
    }

    pub fn get_wind_idx(&self, board: &BoardState, player: Player) -> usize {
        (self._get_wind_idx)(board, player)
    }

    pub fn get_next_states_interactive(&self, state: &FullGameState) -> Vec<BoardStateWithAction> {
        let all_moves = (self._get_all_moves)(state, state.board.current_player, BitBoard::EMPTY);

        // Lose due to no moves
        if all_moves.len() == 0 {
            let mut losing_board = state.board.clone();
            losing_board.set_winner(!losing_board.current_player);

            return vec![BoardStateWithAction::new(
                losing_board,
                vec![PartialAction::NoMoves],
            )];
        }

        all_moves
            .into_iter()
            .flat_map(|action| {
                let mut result_state = state.board.clone();
                self.make_move(&mut result_state, action.action);
                let action_paths = (self._get_actions_for_move)(&state.board, action.action);

                action_paths.into_iter().map(move |full_actions| {
                    BoardStateWithAction::new(result_state.clone(), full_actions)
                })
            })
            .collect()
    }

    pub(crate) fn get_all_moves(&self, state: &FullGameState, player: Player) -> Vec<ScoredMove> {
        (self._get_all_moves)(state, player, BitBoard::EMPTY)
    }

    pub fn get_all_next_states(&self, state: &FullGameState) -> Vec<BoardState> {
        let board = &state.board;
        (self._get_all_moves)(state, board.current_player, BitBoard::EMPTY)
            .into_iter()
            .map(|action| {
                let mut result_state = board.clone();
                self.make_move(&mut result_state, action.action);
                result_state
            })
            .collect()
    }

    pub fn get_moves_for_search(&self, state: &FullGameState, player: Player) -> Vec<ScoredMove> {
        (self._get_moves_for_search)(state, player, BitBoard::EMPTY)
    }

    pub fn get_winning_moves(&self, state: &FullGameState, player: Player) -> Vec<ScoredMove> {
        (self._get_wins)(state, player, BitBoard::EMPTY)
    }

    pub fn get_scored_blocker_moves(
        &self,
        state: &FullGameState,
        player: Player,
        key_moves: BitBoard,
    ) -> Vec<ScoredMove> {
        (self._get_scored_win_blockers)(state, player, key_moves)
    }

    pub fn get_unscored_blocker_moves(
        &self,
        state: &FullGameState,
        player: Player,
        key_moves: BitBoard,
    ) -> Vec<ScoredMove> {
        (self._get_unscored_win_blockers)(state, player, key_moves)
    }

    pub fn get_blocker_board(&self, board: &BoardState, action: GenericMove) -> BitBoard {
        (self._get_blocker_board)(board, action)
    }

    pub fn make_move(&self, board: &mut BoardState, action: GenericMove) {
        (self._make_move)(board, action);
        board.flip_current_player();
    }

    pub fn make_passing_move(&self, board: &mut BoardState) {
        (self._make_passing_move)(board);
        board.flip_current_player();
    }

    pub fn stringify_move(&self, action: GenericMove) -> String {
        (self._stringify_move)(action)
    }

    pub fn get_actions_for_move(&self, board: &BoardState, action: GenericMove) -> Vec<FullAction> {
        (self._get_actions_for_move)(board, action)
    }

    pub(super) fn get_history_hash(&self, board: &BoardState, action: GenericMove) -> usize {
        (self._get_history_hash)(board, action)
    }

    pub(super) fn can_opponent_climb(&self, board: &BoardState, player: Player) -> bool {
        (self._can_opponent_climb_fn)(board, player)
    }

    pub(super) fn get_frozen_mask(&self, board: &BoardState, player: Player) -> BitBoard {
        (self._get_frozen_mask)(board, player)
    }

    pub(super) fn parse_god_data(&self, fen: &str) -> Result<GodData, String> {
        (self._parse_god_data)(fen)
    }

    pub(super) fn stringify_god_data(&self, god_data: GodData) -> Option<String> {
        (self._stringify_god_data)(god_data)
    }

    pub fn pretty_stringify_god_data(&self, board: &BoardState, player: Player) -> Option<String> {
        (self._pretty_stringify_god_data)(board, player)
    }

    pub fn get_flip_horizontal_god_data(&self, god_data: GodData) -> GodData {
        (self._flip_god_data_horizontal)(god_data)
    }

    pub fn get_flip_vertical_god_data(&self, god_data: GodData) -> GodData {
        (self._flip_god_data_vertical)(god_data)
    }

    pub fn get_flip_transpose_god_data(&self, god_data: GodData) -> GodData {
        (self._flip_god_data_transpose)(god_data)
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

counted_array!(pub const ALL_GODS_BY_ID: [GodPower; _] = [
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
    hera::build_hera(),
    limus::build_limus(),
    hypnus::build_hypnus(),
    harpies::build_harpies(),
    aphrodite::build_aphrodite(),
    persephone::build_persephone(),
    hades::build_hades(),
    morpheus::build_morpheus(),
    aeolus::build_aeolus(),
    hestia::build_hestia(),
    europa::build_europa(),
    bia::build_bia(),
    clio::build_clio(),
    maenads::build_maenads(),
]);

#[macro_export]
macro_rules! build_god_power_movers {
    (
        $move_gen:ident
    ) => {{
        {
            crate::gods::GodPowerMoveFns {
                _get_all_moves: $move_gen::<0, false>,
                _get_moves_for_search: $move_gen::<
                    { crate::gods::generic::STOP_ON_MATE | crate::gods::generic::INCLUDE_SCORE },
                    false,
                >,
                _get_wins: $move_gen::<{ crate::gods::generic::MATE_ONLY }, false>,
                _get_scored_win_blockers: $move_gen::<
                    {
                        crate::gods::generic::STOP_ON_MATE
                            | crate::gods::generic::INTERACT_WITH_KEY_SQUARES
                            | crate::gods::generic::INCLUDE_SCORE
                    },
                    false,
                >,
                _get_unscored_win_blockers: $move_gen::<
                    {
                        crate::gods::generic::STOP_ON_MATE
                            | crate::gods::generic::INTERACT_WITH_KEY_SQUARES
                    },
                    false,
                >,
            }
        }
    }};
}

pub(crate) const fn build_god_power_actions<T: GodMove>() -> GodPowerActionFns {
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
        action.make_move(board, board.current_player)
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
        _get_scored_win_blockers: movers._get_scored_win_blockers,
        _get_unscored_win_blockers: movers._get_unscored_win_blockers,

        _build_mask_fn: _default_build_mask,
        _moveable_worker_filter_fn: _default_moveable_worker_filter,
        _can_opponent_climb_fn: _default_can_opponent_climb,
        _get_frozen_mask: _default_get_frozen_mask,

        _flip_god_data_horizontal: _default_flip_god_data,
        _flip_god_data_vertical: _default_flip_god_data,
        _flip_god_data_transpose: _default_flip_god_data,

        _get_wind_idx: _default_get_wind_idx,

        _make_passing_move: _default_passing_move,

        _get_blocker_board: actions._get_blocker_board,
        _get_actions_for_move: actions._get_actions_for_move,
        _make_move: actions._make_move,
        _stringify_move: actions._stringify_move,
        _get_history_hash: actions._get_history_hash,

        _parse_god_data: _default_parse_god_data,
        _stringify_god_data: _default_stringify_god_data,

        _pretty_stringify_god_data: _default_pretty_stringify_god_data,

        num_workers: 2,

        win_mask: BitBoard::MAIN_SECTION_MASK,

        is_aphrodite: false,
        is_persephone: false,
        is_preventing_down: false,
        is_placement_priority: false,

        hash1,
        hash2,
    }
}

impl GodPower {
    pub(super) const fn with_nnue_god_name(mut self, name: GodName) -> Self {
        self.model_god_name = name;
        self
    }

    pub(super) const fn with_num_workers(mut self, num_workers: usize) -> Self {
        self.num_workers = num_workers;
        self
    }

    pub(super) const fn with_win_mask(mut self, win_mask: BitBoard) -> Self {
        self.win_mask = win_mask;
        self
    }

    pub(super) const fn with_build_mask_fn(mut self, build_mask_fn: BuildMaskFn) -> Self {
        self._build_mask_fn = build_mask_fn;
        self
    }

    pub(super) const fn with_is_aphrodite(mut self) -> Self {
        self.is_aphrodite = true;
        self
    }

    pub(super) const fn with_is_persephone(mut self) -> Self {
        self.is_persephone = true;
        self
    }

    pub(super) const fn with_is_preventing_down(mut self) -> Self {
        self.is_preventing_down = true;
        self
    }

    pub(super) const fn with_is_placement_priority(mut self) -> Self {
        self.is_placement_priority = true;
        self
    }

    pub(super) const fn with_moveable_worker_filter(
        mut self,
        moveable_worker_filter_fn: MovableWorkerFilter,
    ) -> Self {
        self._moveable_worker_filter_fn = moveable_worker_filter_fn;
        self
    }

    pub(super) const fn with_can_opponent_climb_fn(
        mut self,
        can_opponent_climb_fn: CanOpponentClimbFn,
    ) -> Self {
        self._can_opponent_climb_fn = can_opponent_climb_fn;
        self
    }

    pub(super) const fn with_make_passing_move_fn(
        mut self,
        make_passing_move: MakePassingMoveFn,
    ) -> Self {
        self._make_passing_move = make_passing_move;
        self
    }

    pub(super) const fn with_parse_god_data_fn(mut self, parse_god_data: ParseGodDataFn) -> Self {
        self._parse_god_data = parse_god_data;
        self
    }

    pub(super) const fn with_stringify_god_data_fn(
        mut self,
        stringify_god_data: StringifyGodDataFn,
    ) -> Self {
        self._stringify_god_data = stringify_god_data;
        self
    }

    pub(super) const fn with_pretty_stringify_god_data_fn(
        mut self,
        stringify_god_data: PrettyStringifyGodDataFn,
    ) -> Self {
        self._pretty_stringify_god_data = stringify_god_data;
        self
    }

    pub(super) const fn with_get_wind_idx_fn(mut self, get_wind_idx_fn: GetWindIdxFn) -> Self {
        self._get_wind_idx = get_wind_idx_fn;
        self
    }

    pub(super) const fn with_get_frozen_mask_fn(
        mut self,
        get_frozen_mask_fn: GetFrozenMask,
    ) -> Self {
        self._get_frozen_mask = get_frozen_mask_fn;
        self
    }

    pub(super) const fn with_flip_god_data_horizontal_fn(
        mut self,
        flip_god_data_fn: FlipGodDataFn,
    ) -> Self {
        self._flip_god_data_horizontal = flip_god_data_fn;
        self
    }

    pub(super) const fn with_flip_god_data_vertical_fn(
        mut self,
        flip_god_data_fn: FlipGodDataFn,
    ) -> Self {
        self._flip_god_data_vertical = flip_god_data_fn;
        self
    }

    pub(super) const fn with_flip_god_data_transpose_fn(
        mut self,
        flip_god_data_fn: FlipGodDataFn,
    ) -> Self {
        self._flip_god_data_transpose = flip_god_data_fn;
        self
    }
}

#[derive(Debug, Default)]
pub(super) struct HistoryIdxHelper(usize);

impl HistoryIdxHelper {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_value(&mut self, current_value: usize, max_value: usize) {
        self.0 = self.0 * max_value + current_value;
    }

    pub(crate) fn add_square_with_height(&mut self, board: &BoardState, square: Square) {
        let height = board.get_height(square);
        self.add_value((square as usize) * 4 + height, 100);
    }

    pub(crate) fn add_maybe_square_with_height(
        &mut self,
        board: &BoardState,
        square: Option<Square>,
    ) {
        if let Some(square) = square {
            let height = board.get_height(square);
            self.add_value((square as usize) * 4 + height + 1, 101);
        } else {
            self.0 *= 101
        }
    }

    pub(crate) fn add_bool(&mut self, value: bool) {
        self.add_value(value as usize, 2);
    }

    pub(crate) fn get(&self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{Rng, rng};

    use crate::fen::parse_fen;

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
        let suggestions: [HashType; 2] =
            [rng.random_range(0..u64::MAX), rng.random_range(0..u64::MAX)];
        let mut all_hashes: HashSet<HashType> = HashSet::new();
        for god_power in ALL_GODS_BY_ID.iter() {
            assert!(
                !all_hashes.contains(&god_power.hash1),
                "hash1 number {} for {:?} is not unique. Here's some suggestions: {:?}",
                god_power.hash1,
                god_power.god_name,
                suggestions
            );
            all_hashes.insert(god_power.hash1);

            assert!(
                !all_hashes.contains(&god_power.hash2),
                "hash2 number {} for {:?} is not unique. Here's some suggestions: {:?}",
                god_power.hash2,
                god_power.god_name,
                suggestions
            );
            all_hashes.insert(god_power.hash2);
        }
    }

    #[test]
    fn test_all_gods_can_lose_with_no_workers() {
        for god_power in ALL_GODS_BY_ID.iter() {
            let fen = format!(
                "10000 00000 00000 00000 00000/1/{}/mortal:A1, A2",
                god_power.god_name
            );
            let state = parse_fen(&fen).unwrap();
            state.get_next_states_interactive();
        }
    }
}
