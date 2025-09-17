use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    gods::{FullAction, PartialAction},
    player::Player,
    square::Square,
};
use std::fmt::Debug;

pub type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const INTERACT_WITH_KEY_SQUARES: MoveGenFlags = 1 << 4;

pub const ANY_MOVE_FILTER: MoveGenFlags = MATE_ONLY | INTERACT_WITH_KEY_SQUARES;

pub const NON_IMPROVER_SENTINEL_SCORE: MoveScore = MoveScore::MIN + 1;
pub const IMPROVER_SENTINEL_SCORE: MoveScore = NON_IMPROVER_SENTINEL_SCORE + 1;
pub const CHECK_SENTINEL_SCORE: MoveScore = IMPROVER_SENTINEL_SCORE + 1;

const SCORE_LOOKUP: [MoveScore; 4] = [
    NON_IMPROVER_SENTINEL_SCORE,
    IMPROVER_SENTINEL_SCORE,
    CHECK_SENTINEL_SCORE,
    CHECK_SENTINEL_SCORE,
];

pub const fn score_lookup(is_check: bool, is_improver: bool) -> MoveScore {
    SCORE_LOOKUP[2 * (is_check as usize) + is_improver as usize]
}

pub const NULL_MOVE_DATA: MoveData = 0;

pub type MoveScore = i16;
pub type MoveData = u32;

pub const MOVE_WINNING_SCORE: MoveScore = MoveScore::MAX;
pub const TT_MATCH_SCORE: MoveScore = MOVE_WINNING_SCORE - 1;
pub const KILLER_MATCH_SCORE: MoveScore = TT_MATCH_SCORE - 1;
pub const LOWEST_SPECIAL_SCORE: MoveScore = KILLER_MATCH_SCORE;

pub const LOWER_POSITION_MASK: u8 = 0b11111;
pub const POSITION_WIDTH: usize = 5;

pub const FULL_HEIGHT_WIDTH: usize = 3;
pub const FULL_HEIGHT_MASK: u8 = (1 << FULL_HEIGHT_WIDTH) - 1;

pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);
pub const MOVE_IS_CHECK_MASK: MoveData = MOVE_IS_WINNING_MASK >> 1;
pub const MOVE_DATA_MAIN_SECTION: MoveData = MOVE_IS_CHECK_MASK - 1;

pub(crate) const _MOVE_IS_WINNING_OFFSET: usize = 31;
const _WINNING_MOVE_ASSERT: () = assert!(1 << _MOVE_IS_WINNING_OFFSET == MOVE_IS_WINNING_MASK);

pub(crate) fn get_default_parse_data_err(data: &str) -> Result<GodData, String> {
    Err(format!("Could not parse god data: {}", data))
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct GenericMove(pub MoveData);

pub trait GodMove: From<GenericMove> + Into<GenericMove> + std::fmt::Debug {
    fn move_to_actions(self, board: &BoardState) -> Vec<FullAction>;

    fn make_move(self, board: &mut BoardState, player: Player);

    fn get_blocker_board(self, board: &BoardState) -> BitBoard;

    fn get_history_idx(self, board: &BoardState) -> usize;
}

#[derive(Copy, Clone, Debug)]
pub struct ScoredMove {
    pub action: GenericMove,
    pub score: MoveScore,
}

impl PartialEq for ScoredMove {
    fn eq(&self, other: &Self) -> bool {
        self.action == other.action
    }
}

impl Eq for ScoredMove {}

impl ScoredMove {
    pub const fn new(action: GenericMove, score: MoveScore) -> Self {
        Self { action, score }
    }

    pub const fn new_winning_move(action: GenericMove) -> Self {
        Self::new(action, MOVE_WINNING_SCORE)
    }

    pub const fn new_checking_move(action: GenericMove) -> Self {
        Self::new(
            GenericMove::new(action.0 | MOVE_IS_CHECK_MASK),
            CHECK_SENTINEL_SCORE,
        )
    }

    pub const fn new_improving_move(action: GenericMove) -> Self {
        Self::new(action, IMPROVER_SENTINEL_SCORE)
    }

    pub const fn new_non_improver(action: GenericMove) -> Self {
        Self::new(action, NON_IMPROVER_SENTINEL_SCORE)
    }

    pub const fn new_unscored_move(action: GenericMove) -> Self {
        Self::new(action, 0)
    }

    pub fn get_is_winning(&self) -> bool {
        self.action.get_is_winning()
    }

    pub fn get_score(&self) -> MoveScore {
        self.score
    }

    pub fn set_score(&mut self, score: MoveScore) {
        self.score = score
    }
}

impl GenericMove {
    pub const NULL_MOVE: GenericMove = GenericMove::new(NULL_MOVE_DATA);

    pub const fn new(data: MoveData) -> Self {
        Self(data)
    }

    pub const fn new_winning_move(data: MoveData) -> Self {
        Self(data | MOVE_IS_WINNING_MASK)
    }

    pub fn set_is_check(&mut self) {
        self.0 |= MOVE_IS_CHECK_MASK;
    }

    pub fn get_is_check(&self) -> bool {
        self.0 & MOVE_IS_CHECK_MASK != 0
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl From<MoveData> for GenericMove {
    fn from(value: MoveData) -> Self {
        Self::new(value)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct WorkerPlacement(pub MoveData);

impl Into<GenericMove> for WorkerPlacement {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for WorkerPlacement {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl WorkerPlacement {
    pub fn new(a: Square, b: Square) -> Self {
        let data: MoveData = ((a as MoveData) << 0)
            | ((b as MoveData) << POSITION_WIDTH)
            | ((25 as MoveData) << 2 * POSITION_WIDTH);

        Self(data)
    }

    pub fn new_3(a: Square, b: Square, c: Square) -> Self {
        let data: MoveData = ((a as MoveData) << 0)
            | ((b as MoveData) << POSITION_WIDTH)
            | ((c as MoveData) << 2 * POSITION_WIDTH);

        Self(data)
    }

    pub fn placement_1(self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn placement_2(self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn placement_3(self) -> Option<Square> {
        let value = (self.0 >> 2 * POSITION_WIDTH) as u8 & LOWER_POSITION_MASK;
        if value < 25 {
            Some(Square::from(value))
        } else {
            None
        }
    }

    pub fn move_mask(self) -> BitBoard {
        if let Some(placement_3) = self.placement_3() {
            BitBoard::as_mask(self.placement_1())
                | BitBoard::as_mask(self.placement_2())
                | BitBoard::as_mask(placement_3)
        } else {
            BitBoard::as_mask(self.placement_1()) | BitBoard::as_mask(self.placement_2())
        }
    }

    pub fn make_on_clone(self, state: &FullGameState, player: Player) -> FullGameState {
        let mut result = state.clone();
        self.make_move(&mut result.board, player);
        result
    }
}

impl std::fmt::Debug for WorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(placement_3) = self.placement_3() {
            write!(
                f,
                "P{} P{} P{}",
                self.placement_1(),
                self.placement_2(),
                placement_3
            )
        } else {
            write!(f, "P{} P{}", self.placement_1(), self.placement_2())
        }
    }
}

impl GodMove for WorkerPlacement {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut actions = vec![
            PartialAction::PlaceWorker(self.placement_1()),
            PartialAction::PlaceWorker(self.placement_2()),
        ];

        if let Some(placement_3) = self.placement_3() {
            actions.push(PartialAction::PlaceWorker(placement_3));
        }

        let actions_len = actions.len();

        let result: Vec<FullAction> = actions.into_iter().permutations(actions_len).collect();

        result
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        board.worker_xor(player, self.move_mask());
        board.flip_current_player();
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, _board: &BoardState) -> usize {
        self.0 as usize
    }
}
