use crate::utils::grid_position_builder;

// TODO: bitflags?
pub type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const RETURN_FIRST_MATE: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;

const POSITION_SCORE_MULT: MoveScore = 1;
pub const GRID_POSITION_SCORES: [MoveScore; 25] = grid_position_builder(
    0 * POSITION_SCORE_MULT,
    1 * POSITION_SCORE_MULT,
    2 * POSITION_SCORE_MULT,
    3 * POSITION_SCORE_MULT,
    4 * POSITION_SCORE_MULT,
    5 * POSITION_SCORE_MULT,
);

const WORKER_HEIGHT_OFFSET: MoveScore = 50;
pub const WORKER_HEIGHT_SCORES: [MoveScore; 4] = [
    0 * WORKER_HEIGHT_OFFSET,
    100 * WORKER_HEIGHT_OFFSET,
    300 * WORKER_HEIGHT_OFFSET,
    101 * WORKER_HEIGHT_OFFSET,
];

pub type MoveScore = i16;
pub type MoveData = u32;
pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);

pub const MOVE_WINNING_SCORE: MoveScore = MoveScore::MAX;
pub const TT_MATCH_SCORE: MoveScore = MOVE_WINNING_SCORE - 1;
pub const KILLER_MATCH_SCORE: MoveScore = TT_MATCH_SCORE - 1;
pub const LOWEST_SPECIAL_SCORE: MoveScore = KILLER_MATCH_SCORE;

pub const LOWER_POSITION_MASK: u8 = 0b11111;
pub const POSITION_WIDTH: usize = 5;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GenericMove {
    pub score: MoveScore,
    pub data: u32,
}

impl PartialEq for GenericMove {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}


impl GenericMove {
    pub const NULL_MOVE: GenericMove = GenericMove::new(0);

    pub const fn new(data: MoveData) -> Self {
        Self { score: 0, data }
    }

    pub const fn new_winning_move(data: MoveData) -> Self {
        Self {
            score: MoveScore::MAX,
            data: data | MOVE_IS_WINNING_MASK,
        }
    }
    pub fn get_score(&self) -> MoveScore {
        self.score
    }

    pub fn set_score(&mut self, score: MoveScore) {
        self.score = score
    }

    pub fn set_is_winning(&mut self) {
        self.data |= MOVE_IS_WINNING_MASK;
    }

    pub fn get_is_winning(&self) -> bool {
        (self.data & MOVE_IS_WINNING_MASK) != 0
    }
}

impl From<MoveData> for GenericMove {
    fn from(value: MoveData) -> Self {
        Self::new(value)
    }
}
