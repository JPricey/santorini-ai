use colored::Colorize;

use crate::{
    fen::{game_state_to_fen, parse_fen},
    gods::{ALL_GODS_BY_ID, GameStateWithAction, GodName, GodPower},
};

use super::search::Hueristic;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Player {
    One = 0,
    Two = 1,
}

impl Default for Player {
    fn default() -> Self {
        Player::One
    }
}

impl Player {
    pub fn other(&self) -> Player {
        match self {
            Player::One => Player::Two,
            Player::Two => Player::One,
        }
    }

    pub fn color(&self) -> Hueristic {
        match &self {
            Player::One => 1,
            Player::Two => -1,
        }
    }
}

pub type BitmapType = u32;
// const NUM_LEVELS: usize = 4;
pub const BOARD_WIDTH: usize = 5;
pub const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

pub const IS_WINNER_MASK: BitmapType = 1 << 31;
pub const MAIN_SECTION_MASK: BitmapType = (1 << 25) - 1;

pub const NEIGHBOR_MAP: [BitmapType; NUM_SQUARES] = [
    98, 229, 458, 916, 776, 3139, 7335, 14670, 29340, 24856, 100448, 234720, 469440, 938880,
    795392, 3214336, 7511040, 15022080, 30044160, 25452544, 2195456, 5472256, 10944512, 21889024,
    9175040,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Serialize for Coord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl<'de> Deserialize<'de> for Coord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        if s.len() != 2 {
            return Err(serde::de::Error::custom("Invalid Coord format"));
        }
        let raw_row = s.char_indices().nth(0).ok_or(serde::de::Error::custom(
            "Invalid Coord format: no row found",
        ))?;
        let x = match raw_row.1 {
            'A' => 0,
            'B' => 1,
            'C' => 2,
            'D' => 3,
            'E' => 4,
            _ => return Err(serde::de::Error::custom("Invalid column letter")),
        };
        let raw_col = s.char_indices().nth(1).ok_or(serde::de::Error::custom(
            "Invalid Coord format: no column found",
        ))?;
        let y = raw_col.1.to_digit(10).ok_or(serde::de::Error::custom(
            "Invalid Coord format: column must be a digit",
        ))? as usize;
        if y < 1 || y > 5 {
            return Err(serde::de::Error::custom("Row must be between 1 and 5"));
        }

        Ok(Coord { x, y: 5 - y })
    }
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Coord { x, y }
    }
}

impl Default for Coord {
    fn default() -> Self {
        Coord { x: 0, y: 0 }
    }
}

impl std::fmt::Debug for Coord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let col = match self.x {
            0 => "A",
            1 => "B",
            2 => "C",
            3 => "D",
            4 => "E",
            _ => panic!("Unknown Column: {}", self.x),
        };
        let row = 5 - self.y;
        write!(f, "{}{}", col, row)
    }
}

pub fn position_to_coord(position: usize) -> Coord {
    let x = position % 5;
    let y = position / 5;
    Coord::new(x, y)
}

#[allow(unused)]
pub fn coord_to_position(coord: Coord) -> usize {
    coord.x + coord.y * BOARD_WIDTH
}

#[allow(unused)]
fn print_full_bitmap(mut mask: BitmapType) {
    for _ in 0..5 {
        let lower = mask & 0b11111;
        let output = format!("{:05b}", lower);
        eprintln!("{}", output.chars().rev().collect::<String>());
        mask = mask >> 5;
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FullGameState {
    pub p1_god: &'static GodPower,
    pub p2_god: &'static GodPower,
    pub board: BoardState,
}

impl Serialize for FullGameState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let fen = game_state_to_fen(self);
        serializer.serialize_str(&fen)
    }
}

impl<'de> Deserialize<'de> for FullGameState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fen: String = Deserialize::deserialize(deserializer)?;
        parse_fen(&fen).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Debug for FullGameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", game_state_to_fen(self))
    }
}

impl TryFrom<&str> for FullGameState {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl TryFrom<&String> for FullGameState {
    type Error = String;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl FullGameState {
    pub fn new(
        board_state: BoardState,
        p1_god: &'static GodPower,
        p2_god: &'static GodPower,
    ) -> Self {
        FullGameState {
            p1_god,
            p2_god,
            board: board_state,
        }
    }

    pub fn new_basic_state(p1: GodName, p2: GodName) -> Self {
        FullGameState::new(BoardState::new_basic_state(), p1.to_power(), p2.to_power())
    }

    pub fn new_basic_state_mortals() -> Self {
        FullGameState::new_basic_state(GodName::Mortal, GodName::Mortal)
    }
}

/*
 * Bitmap of each level:
 * 0  1  2  3  4
 * 5  6  7  8  9
 * 10 11 12 13 14
 * 15 16 17 18 19
 * 20 21 22 23 24
 *
 * bits 25-31 are unclaimed.
 */
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct BoardState {
    pub current_player: Player,
    // height_map[L - 1][s] represents if square s is GTE L
    pub height_map: [BitmapType; 4],
    pub workers: [BitmapType; 2],
}

impl BoardState {
    pub fn player_1_god(&self) -> &'static GodPower {
        &ALL_GODS_BY_ID[0]
    }

    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[1] |= 1 << 7;
        result.workers[1] |= 1 << 17;
        result.workers[0] |= 1 << 12;
        result.workers[0] |= 1 << 13;
        result
    }

    pub fn flip_current_player(&mut self) {
        self.current_player = self.current_player.other()
    }

    pub fn get_height_for_worker(&self, worker_mask: BitmapType) -> usize {
        for h in (0..3).rev() {
            if self.height_map[h] & worker_mask > 0 {
                return h + 1;
            }
        }
        0
    }

    pub fn get_true_height(&self, position_mask: BitmapType) -> usize {
        for h in (0..4).rev() {
            if self.height_map[h] & position_mask > 0 {
                return h + 1;
            }
        }
        0
    }

    pub fn get_winner(&self) -> Option<Player> {
        if self.workers[0] & IS_WINNER_MASK > 0 {
            Some(Player::One)
        } else if self.workers[1] & IS_WINNER_MASK > 0 {
            Some(Player::Two)
        } else {
            None
        }
    }

    pub fn set_winner(&mut self, player: Player) {
        let player_idx = player as usize;
        self.workers[player_idx] |= IS_WINNER_MASK;
    }

    /*
    pub fn get_path_to_outcome(&self, other: &SantoriniState) -> Option<FullAction> {
        for choice in (self.player_1_god().next_states_interactive)() {
            if &choice.result_state == other {
                return Some(choice.actions);
            }
        }

        None
    }
    */

    pub fn print_to_console(&self) {
        // TODO!
        // eprintln!("{:?}", self);

        if let Some(winner) = self.get_winner() {
            eprintln!("Player {:?} wins!", winner);
        } else {
            eprintln!("Player {:?} to play", self.current_player);
        }

        for row in 0..5 {
            let mut row_str = format!("{} ", 5 - row);
            for col in 0..5 {
                let pos = col + row * 5;
                let mask = 1 << pos;
                let height = self.get_true_height(1 << pos);

                let is_1 = self.workers[0] & mask > 0;
                let is_2 = self.workers[1] & mask > 0;

                assert!(
                    !(is_1 && is_2),
                    "A square cannot have both players' workers"
                );

                let char = if is_1 {
                    "X"
                } else if is_2 {
                    "0"
                } else {
                    " "
                }
                .black();

                let elem = match height {
                    0 => char.on_white(),
                    1 => char.on_yellow(),
                    2 => char.on_blue(),
                    3 => char.on_green(),
                    4 => char.on_black(),
                    _ => panic!("Invalid Height: {}", height),
                };
                row_str = format!("{row_str}{elem}");
            }
            eprint!("{}", row_str);
            eprintln!()
        }
        eprintln!(" ABCDE");
    }

    pub fn get_positions_for_player(&self, player: Player) -> Vec<usize> {
        let mut result = Vec::with_capacity(2);
        let mut workers_mask = self.workers[player as usize] & MAIN_SECTION_MASK;

        while workers_mask != 0 {
            let pos = workers_mask.trailing_zeros() as usize;
            result.push(pos);
            workers_mask ^= 1 << pos;
        }

        result
    }
}

pub fn get_next_states_interactive(state: &FullGameState) -> Vec<GameStateWithAction> {
    let active_god = match state.board.current_player {
        Player::One => state.p1_god,
        Player::Two => state.p2_god,
    };
    let board_states_with_action_list =
        (active_god.next_states_interactive)(&state.board, state.board.current_player);
    board_states_with_action_list
        .into_iter()
        .map(|e| GameStateWithAction::new(e, state.p1_god.god_name, state.p2_god.god_name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_coord() {
        for position in 0..25 {
            let coord = position_to_coord(position);
            let coord_str = serde_json::to_string(&coord).unwrap();
            let parsed_coord: Coord = serde_json::from_str(&coord_str).unwrap();

            assert_eq!(coord, parsed_coord);
        }
    }
}
