use std::{fmt, str::FromStr};

use crate::bitboard::BitBoard;
use crate::transmute_enum;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
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
            .position(|&tgt| tgt == s.to_lowercase())
            .ok_or("Invalid square!")?;

        Ok(Square::from(index))
    }
}

impl From<usize> for Square {
    fn from(index: usize) -> Self {
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
        "a5", "b5", "c5", "d5", "e5",
        "a4", "b4", "c4", "d4", "e4",
        "a3", "b3", "c3", "d3", "e3",
        "a2", "b2", "c2", "d2", "e2",
        "a1", "b1", "c1", "d1", "e1",
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
