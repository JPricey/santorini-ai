use crate::utils::grid_position_builder;

// TODO: bitflags?
pub type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const RETURN_FIRST_MATE: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;

pub const GRID_POSITION_SCORES: [u8; 25] = grid_position_builder(0, 1, 2, 3, 4, 5);
pub const WORKER_HEIGHT_SCORES: [u8; 4] = [0, 20, 50, 21];

pub type MoveScore = u8;
pub type MoveData = u32;
pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);

pub const MOVE_WINNING_SCORE: MoveScore = MoveScore::MAX;
pub const TT_MATCH_SCORE: MoveScore = MOVE_WINNING_SCORE - 1;
pub const KILLER_MATCH_SCORE: MoveScore = TT_MATCH_SCORE - 1;

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
    pub fn new(data: MoveData) -> Self {
        Self { score: 0, data }
    }

    pub fn new_winning_move(data: MoveData) -> Self {
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
