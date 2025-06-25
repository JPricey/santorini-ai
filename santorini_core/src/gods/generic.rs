use crate::{board::BoardState, player::Player};

// TODO: bitflags?
type MoveGenFlags = u8;
const STOP_ON_MATE: MoveGenFlags = 1 << 0;
const INCLUDE_CHECKS: MoveGenFlags = 1 << 1;
const INCLUDE_QUIET: MoveGenFlags = 1 << 1;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GenericMove {
    data: u64,
}

// Mortal moves are represented as:
// [25b: worker move mask][7b - space][8b build position][4b build height][...score]

// TODO: accept a move accumulator and use that instead of returning a vec
pub fn mortal_move_gen<const F: MoveGenFlags>(state: &BoardState, player: Player) -> Vec<GenericMove> {
    let mut result: Vec<GenericMove> = Vec::with_capacity(128);
    let current_player_idx = player as usize;

    result
}
