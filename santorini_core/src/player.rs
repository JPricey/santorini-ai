use std::ops::Not;

use crate::transmute_enum_masked;

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

    fn not(self) -> Self {
        transmute_enum_masked!(self as u8 ^ 1, 1)
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
}
