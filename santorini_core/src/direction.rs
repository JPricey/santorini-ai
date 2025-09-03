use crate::{square::Square, transmute_enum};

#[repr(u8)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Direction {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Direction {
    pub const fn to_icoord(self) -> ICoord {
        match self {
            Direction::N => ICoord::new(0, -1),
            Direction::NE => ICoord::new(1, -1),
            Direction::E => ICoord::new(1, 0),
            Direction::SE => ICoord::new(1, 1),
            Direction::S => ICoord::new(0, 1),
            Direction::SW => ICoord::new(-1, 1),
            Direction::W => ICoord::new(-1, 0),
            Direction::NW => ICoord::new(-1, -1),
        }
    }

    pub const fn from_u8(val: u8) -> Self {
        transmute_enum!(val)
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct ICoord {
    pub col: i32,
    pub row: i32,
}

impl ICoord {
    pub const fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }

    pub const fn is_in_bound(self) -> bool {
        self.col >= 0 && self.col < 5 && self.row >= 0 && self.row < 5
    }

    pub const fn to_square(self) -> Option<Square> {
        if self.is_in_bound() {
            Some(Square::from_col_row(self.col as usize, self.row as usize))
        } else {
            None
        }
    }

    pub const fn add(self, other: Self) -> Self {
        Self {
            col: self.col + other.col,
            row: self.row + other.row,
        }
    }

    pub const fn sub(self, other: Self) -> Self {
        Self {
            col: self.col - other.col,
            row: self.row - other.row,
        }
    }
}
