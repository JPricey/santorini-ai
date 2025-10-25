use crate::{
    board::{BoardState, GodPair},
    player::Player,
};

pub(crate) mod common;
pub(crate) mod opposite;
pub(crate) mod perimeter;
pub(crate) mod standard;
pub(crate) mod three_worker;

pub type MaybePlacementState = Option<PlacementState>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementType {
    Normal,
    ThreeWorkers,
    PerimeterOnly,
    PerimeterOpposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementState {
    pub next_placement: Player,
    pub is_swapped: bool,
}

impl PlacementState {
    pub fn new(next_placement: Player, is_swapped: bool) -> Self {
        Self {
            next_placement,
            is_swapped,
        }
    }

    pub fn next(self) -> Option<PlacementState> {
        match (self.is_swapped, self.next_placement) {
            (false, Player::One) => Some(PlacementState::new(Player::Two, false)),
            (false, Player::Two) => None,

            (true, Player::Two) => Some(PlacementState::new(Player::One, true)),
            (true, Player::One) => None,
        }
    }
}

pub fn get_starting_placement_state(
    board: &BoardState,
    gods: GodPair,
) -> Result<MaybePlacementState, String> {
    // If the board has changed at all, assume the game as started
    if board.height_map[0].is_not_empty() {
        return Ok(None);
    }

    let is_placement_flipped = gods[1].is_placement_priority && !gods[0].is_placement_priority;

    let p1_is_placed = board.workers[0].is_not_empty();
    let p2_is_placed = board.workers[1].is_not_empty();

    if is_placement_flipped {
        match (p1_is_placed, p2_is_placed) {
            (true, true) => Ok(None),
            (false, false) => Ok(Some(PlacementState::new(Player::Two, true))),
            (false, true) => Ok(Some(PlacementState::new(Player::One, true))),
            (true, false) => Err( "Invalid starting position. Player 1 has placed workers, but expected Player 2 to place first" .to_owned(),
            ),
        }
    } else {
        match (p1_is_placed, p2_is_placed) {
            (true, true) => Ok(None),
            (false, false) => Ok(Some(PlacementState::new(Player::One, false))),
            (true, false) => Ok(Some(PlacementState::new(Player::Two, false))),
            (false, true) => Err( "Invalid starting position. Player 2 has placed workers, but expected Player 1 to place first" .to_owned()),
        }
    }
}
