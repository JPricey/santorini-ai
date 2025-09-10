use crate::{square::Square, transmute_enum};

#[repr(u8)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Direction {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Direction {
    pub const fn to_icoord(self) -> ICoord {
        match self {
            Direction::NW => ICoord::new(-1, -1),
            Direction::N => ICoord::new(0, -1),
            Direction::NE => ICoord::new(1, -1),
            Direction::E => ICoord::new(1, 0),
            Direction::SE => ICoord::new(1, 1),
            Direction::S => ICoord::new(0, 1),
            Direction::SW => ICoord::new(-1, 1),
            Direction::W => ICoord::new(-1, 0),
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

fn directionm_to_delta(direction: Direction) -> i32 {
    match direction {
        Direction::NW => -6,
        Direction::N => -5,
        Direction::NE => -4,
        Direction::E => 1,
        Direction::SE => 6,
        Direction::S => 5,
        Direction::SW => 4,
        Direction::W => -1,
    }
}

pub fn squares_to_direction(start: Square, end: Square) -> Direction {
    let delta = end as i32 - start as i32;

    match delta {
        -6 => Direction::NW,
        -5 => Direction::N,
        -4 => Direction::NE,
        1 => Direction::E,
        6 => Direction::SE,
        5 => Direction::S,
        4 => Direction::SW,
        -1 => Direction::W,
        _ => panic!("Squares are not adjacent"),
    }
}

pub fn offset_square_by_dir(square: Square, direction: Direction) -> Square {
    let delta = directionm_to_delta(direction);
    let new_square = square as i32 + delta;
    debug_assert!(new_square >= 0 && new_square < 25);
    Square::from(new_square as u8)
}
