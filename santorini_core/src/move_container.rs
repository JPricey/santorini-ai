pub type MoveScore = u8;
pub type MoveData = u32;
pub const MOVE_IS_WINNING_MASK: MoveData = MoveData::MAX ^ (MoveData::MAX >> 1);

pub const MOVE_WINNING_SCORE: MoveScore = MoveScore::MAX;
pub const TT_MATCH_SCORE: MoveScore = MOVE_WINNING_SCORE - 1;

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

// Somehow sharing this between multiple levels is slower than creating a new vec each time.
// Pretty amazing. Not sure how.
pub struct ParentMoveContainer {
    pub moves: Box<[GenericMove]>,
}

impl ParentMoveContainer {
    pub fn new(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        unsafe {
            vec.set_len(capacity);
        }

        Self {
            moves: vec.into_boxed_slice(),
        }
    }

    pub fn get_child(&mut self) -> ChildMoveContainer {
        ChildMoveContainer {
            parent_move_container: self,
            head: 0,
            tail: 0,
        }
    }
}

impl Default for ParentMoveContainer {
    fn default() -> Self {
        Self::new(65536)
    }
}

pub struct ChildMoveContainer<'a> {
    parent_move_container: &'a mut ParentMoveContainer,
    head: usize,
    tail: usize,
}

impl<'a> ChildMoveContainer<'a> {
    pub fn get_child(&mut self) -> ChildMoveContainer {
        ChildMoveContainer {
            parent_move_container: self.parent_move_container,
            head: self.tail,
            tail: self.tail,
        }
    }

    pub fn push(&mut self, action: GenericMove) {
        self.parent_move_container.moves[self.tail] = action;
        self.tail += 1;
    }

    pub fn consume(&mut self) -> Option<GenericMove> {
        if self.tail <= self.head {
            None
        } else {
            self.tail -= 1;
            Some(self.parent_move_container.moves[self.tail])
        }
    }
}
