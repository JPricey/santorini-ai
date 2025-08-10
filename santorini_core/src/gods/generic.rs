use serde::{Deserialize, Serialize};

use crate::{bitboard::BitBoard, board::BoardState, gods::{FullAction, PartialAction}, square::Square, utils::grid_position_builder};
use std::fmt::Debug;

pub type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const INTERACT_WITH_KEY_SQUARES: MoveGenFlags = 1 << 4;
pub const GENERATE_THREATS_ONLY: MoveGenFlags = 1 << 5;

pub const NON_IMPROVER_SENTINEL_SCORE: MoveScore = MoveScore::MIN + 1;
pub const IMPROVER_SENTINEL_SCORE: MoveScore = NON_IMPROVER_SENTINEL_SCORE + 1;
pub const CHECK_SENTINEL_SCORE: MoveScore = IMPROVER_SENTINEL_SCORE + 1;

pub const NULL_MOVE_DATA: MoveData = 0;

const POSITION_SCORE_MULT: MoveScore = 1;
pub const GRID_POSITION_SCORES: [MoveScore; 25] = grid_position_builder(
    0 * POSITION_SCORE_MULT,
    2 * POSITION_SCORE_MULT,
    1 * POSITION_SCORE_MULT,
    6 * POSITION_SCORE_MULT,
    7 * POSITION_SCORE_MULT,
    8 * POSITION_SCORE_MULT,
);

const WORKER_HEIGHT_COEFF: MoveScore = 1;
pub const WORKER_HEIGHT_SCORES: [MoveScore; 4] = [
    0 * WORKER_HEIGHT_COEFF,
    30 * WORKER_HEIGHT_COEFF,
    100 * WORKER_HEIGHT_COEFF,
    31 * WORKER_HEIGHT_COEFF,
];

pub const IMPROVER_BUILD_HEIGHT_SCORES: [[MoveScore; 4]; 4] = [
    [0, 0, 0, 0],
    [8, 45, -388, 0],
    [3, 14, 69, -800],
    [0, 0, 0, 0],
];

pub const ENEMY_WORKER_BUILD_SCORES: [[MoveScore; 5]; 4] = [
    [-111, 39, 41, 80, 0],
    [-40, -100, 299, 400, 0],
    [-8, -80, -10000, 12000, 0],
    [0, 0, 0, 0, 0],
];
pub const CHECK_MOVE_BONUS: MoveScore = 8000;

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

// A move will be
// 5 bits from
// 5 bits to
// 1 bit win?
// 5 bits build
// > 16 bits
// would be nice to include some metadata about heights and stuff, but whatever

pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);
pub const MOVE_IS_CHECK_MASK: MoveData = MOVE_IS_WINNING_MASK >> 1;

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct GenericMove(pub MoveData);

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
        Self {
            action,
            score: MOVE_WINNING_SCORE,
        }
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
        let data: MoveData = ((a as MoveData) << 0) | ((b as MoveData) << POSITION_WIDTH);

        Self(data)
    }

    pub fn placement_1(self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn placement_2(self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.placement_1()) | BitBoard::as_mask(self.placement_2())
    }

    pub fn make_move(self, board: &mut BoardState) {
        board.worker_xor(board.current_player, self.move_mask());
        board.current_player = !board.current_player;
    }

    pub fn unmake_move(self, board: &mut BoardState) {
        board.current_player = !board.current_player;
        board.worker_xor(board.current_player, self.move_mask());
    }

    pub fn move_to_actions(self) -> Vec<FullAction> {
        return vec![
            vec![
                PartialAction::PlaceWorker(self.placement_1()),
                PartialAction::PlaceWorker(self.placement_2()),
            ],
            vec![
                PartialAction::PlaceWorker(self.placement_2()),
                PartialAction::PlaceWorker(self.placement_1()),
            ],
        ]
    }
}

impl std::fmt::Debug for WorkerPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{} P{}", self.placement_1(), self.placement_2())
    }
}
