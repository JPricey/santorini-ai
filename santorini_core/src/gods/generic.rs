use serde::{Deserialize, Serialize};

use crate::utils::grid_position_builder;
use std::fmt::Debug;

pub type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const RETURN_FIRST_MATE: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;

pub const NON_IMPROVER_SENTINEL_SCORE: MoveScore = MoveScore::MIN + 1;
pub const IMPROVER_SENTINEL_SCORE: MoveScore = NON_IMPROVER_SENTINEL_SCORE + 1;

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

pub type MoveScore = i16;
pub type MoveData = u16;

pub const MOVE_WINNING_SCORE: MoveScore = MoveScore::MAX;
pub const TT_MATCH_SCORE: MoveScore = MOVE_WINNING_SCORE - 1;
pub const KILLER_MATCH_SCORE: MoveScore = TT_MATCH_SCORE - 1;
pub const LOWEST_SPECIAL_SCORE: MoveScore = KILLER_MATCH_SCORE;

pub const LOWER_POSITION_MASK: u8 = 0b11111;
pub const POSITION_WIDTH: usize = 5;

// A move will be
// 5 bits from
// 5 bits to
// 1 bit win?
// 5 bits build
// > 16 bits
// would be nice to include some metadata about heights and stuff, but whatever

pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericMove(pub MoveData);

#[derive(Copy, Clone, Debug)]
pub struct ScoredMove {
    pub action: GenericMove,
    pub score: MoveScore,
}

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
    pub const NULL_MOVE: GenericMove = GenericMove::new(0);

    pub const fn new(data: MoveData) -> Self {
        Self(data)
    }

    pub const fn new_winning_move(data: MoveData) -> Self {
        Self(data | MOVE_IS_WINNING_MASK)
    }

    // pub fn set_is_winning(&mut self) {
    //     self.0 |= MOVE_IS_WINNING_MASK;
    // }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl From<MoveData> for GenericMove {
    fn from(value: MoveData) -> Self {
        Self::new(value)
    }
}

impl Debug for GenericMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == GenericMove::NULL_MOVE {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.mortal_build_position();
        let is_win = self.get_is_winning();

        if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}
