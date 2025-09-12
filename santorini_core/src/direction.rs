use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

use crate::{square::Square, transmute_enum};

#[repr(u8)]
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

    pub const fn wrap_in_bounds(self) -> Self {
        Self {
            col: (self.col % 5 + 5) % 5,
            row: (self.row % 5 + 5) % 5,
        }
    }
}

fn _direction_to_delta(direction: Direction) -> i32 {
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

pub fn direction_to_ui_square(direction: Direction) -> Square {
    match direction {
        Direction::NW => Square::A5,
        Direction::N => Square::C5,
        Direction::NE => Square::E5,
        Direction::E => Square::E3,
        Direction::SE => Square::E1,
        Direction::S => Square::C1,
        Direction::SW => Square::A1,
        Direction::W => Square::A3,
    }
}

pub fn maybe_wind_direction_to_square(direction: Option<Direction>) -> Square {
    direction.map_or(Square::C3, direction_to_ui_square)
}

pub(crate) fn _offset_square_by_dir(square: Square, direction: Direction) -> Square {
    let delta = _direction_to_delta(direction);
    let new_square = square as i32 + delta;
    debug_assert!(new_square >= 0 && new_square < 25);
    Square::from(new_square as u8)
}

// Where 0 is nothing, 1-8 is a direction index
pub(crate) fn direction_idx_to_reverse(direction_idx: usize) -> usize {
    match direction_idx {
        0 => 0,
        1..=4 => direction_idx + 4,
        5..=8 => direction_idx - 4,
        9.. => unreachable!(),
    }
}
