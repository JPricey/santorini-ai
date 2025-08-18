use std::{fmt, str::FromStr};

use crate::bitboard::BitBoard;
use crate::transmute_enum;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, EnumIter)]
#[rustfmt::skip]
pub enum Square {
    A5, B5, C5, D5, E5,
    A4, B4, C4, D4, E4,
    A3, B3, C3, D3, E3,
    A2, B2, C2, D2, E2,
    A1, B1, C1, D1, E1,
}
use Square::*;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

impl Square {
    pub const fn from_col_row(col: usize, row: usize) -> Square {
        transmute_enum!((col + 5 * row) as u8)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = String::from(Self::STR[*self as usize]);
        write!(f, "{s}")
    }
}

impl FromStr for Square {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(pos) = s.parse::<usize>() {
            if pos < Self::COUNT {
                return Ok(Square::from(pos));
            }
        }

        if s.len() != 2 {
            return Err("Invalid square!");
        };

        let index = Self::STR
            .iter()
            .position(|&tgt| tgt.to_lowercase() == s.to_lowercase())
            .ok_or("Invalid square!")?;

        Ok(Square::from(index))
    }
}

impl From<usize> for Square {
    fn from(index: usize) -> Self {
        transmute_enum!(index as u8)
    }
}

impl From<u8> for Square {
    fn from(index: u8) -> Self {
        transmute_enum!(index as u8)
    }
}

impl From<u16> for Square {
    fn from(index: u16) -> Self {
        transmute_enum!(index as u8)
    }
}

impl Square {
    pub const COUNT: usize = 25;

    #[rustfmt::skip]
    pub const ALL: [Self; Self::COUNT] = [
        A5, B5, C5, D5, E5,
        A4, B4, C4, D4, E4,
        A3, B3, C3, D3, E3,
        A2, B2, C2, D2, E2,
        A1, B1, C1, D1, E1,
    ];

    #[rustfmt::skip]
    const STR: [&str; Self::COUNT] = [
        "A5", "B5", "C5", "D5", "E5",
        "A4", "B4", "C4", "D4", "E4",
        "A3", "B3", "C3", "D3", "E3",
        "A2", "B2", "C2", "D2", "E2",
        "A1", "B1", "C1", "D1", "E1",
    ];

    pub const fn to_board(self) -> BitBoard {
        BitBoard(1u32 << self as usize)
    }
}

impl Serialize for Square {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Square {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Square::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_square() {
        for position in 0_usize..25 {
            let square = Square::from(position);
            let square_str = serde_json::to_string(&square).unwrap();
            let parsed_square: Square = serde_json::from_str(&square_str).unwrap();

            assert_eq!(square, parsed_square);
        }
    }
}
