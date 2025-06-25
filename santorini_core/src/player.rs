use std::ops::Not;

use crate::{search::Hueristic, transmute_enum, transmute_enum_masked};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Player {
    One,
    Two,
}

impl Default for Player {
    fn default() -> Self {
        Player::One
    }
}

impl Not for Player {
    type Output = Player;

    // get opposite color
    fn not(self) -> Self {
        transmute_enum_masked!(self as u8 ^ 1, 1)
    }
}

impl Player {
    pub fn color(self) -> Hueristic {
        let as_u8: u8 = transmute_enum!(self as u8);
        let as_heuristic: Hueristic = as_u8 as Hueristic;
        (-as_heuristic) * 2 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_negation() {
        let player = Player::One;
        assert_eq!(!player, Player::Two);
        assert_eq!(!!player, Player::One);
    }

    #[test]
    fn test_player_color() {
        assert_eq!(Player::One.color(), 1);
        assert_eq!(Player::Two.color(), -1);
    }
}
