use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodPair},
    gods::generic::{GodMove, WorkerPlacement},
    player::Player,
    square::Square,
};

pub type MaybePlacementState = Option<PlacementState>;

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

pub fn get_all_placements(board: &BoardState, player: Player) -> Vec<WorkerPlacement> {
    debug_assert!(
        board.workers[player as usize] == BitBoard::EMPTY,
        "{:?}",
        board
    );
    let mut res = Vec::new();

    for a in 0_usize..25 {
        let a_sq = Square::from(a);
        if (board.workers[!player as usize] & BitBoard::as_mask(a_sq)).is_not_empty() {
            continue;
        }

        for b in a + 1..25 {
            let b_sq = Square::from(b);
            if (board.workers[!player as usize] & BitBoard::as_mask(b_sq)).is_not_empty() {
                continue;
            }

            let action = WorkerPlacement::new(a_sq, b_sq);
            res.push(action);
        }
    }

    res
}

pub fn get_all_placements_3(board: &BoardState, player: Player) -> Vec<WorkerPlacement> {
    debug_assert!(board.workers[player as usize] == BitBoard::EMPTY);
    let mut res = Vec::new();

    for a in 0_usize..25 {
        let a_sq = Square::from(a);
        if (board.workers[!player as usize] & BitBoard::as_mask(a_sq)).is_not_empty() {
            continue;
        }

        for b in a + 1..25 {
            let b_sq = Square::from(b);
            if (board.workers[!player as usize] & BitBoard::as_mask(b_sq)).is_not_empty() {
                continue;
            }

            for c in b + 1..25 {
                let c_sq = Square::from(c);
                if (board.workers[!player as usize] & BitBoard::as_mask(c_sq)).is_not_empty() {
                    continue;
                }

                let action = WorkerPlacement::new_3(a_sq, b_sq, c_sq);
                res.push(action);
            }
        }
    }

    res
}

pub fn get_unique_placements(state: &FullGameState, player: Player) -> Vec<WorkerPlacement> {
    let mut res = Vec::new();
    let mut unique_boards = Vec::new();

    let placements = get_all_placements(&state.board, player);
    for p in placements {
        let mut b_clone = state.board.clone();
        p.make_move(&mut b_clone, player);
        let mut is_new = true;
        for permutation in b_clone.get_all_permutations::<true>(state.gods, state.base_hash()) {
            if unique_boards.contains(&permutation) {
                is_new = false;
                break;
            }
        }
        if is_new {
            unique_boards.push(b_clone.clone());
            res.push(p);
        }
    }

    res
}

pub fn get_unique_placements_3(state: &FullGameState, player: Player) -> Vec<WorkerPlacement> {
    let mut res = Vec::new();
    let mut unique_boards = Vec::new();

    let placements = get_all_placements_3(&state.board, player);
    for p in placements {
        let mut b_clone = state.board.clone();
        p.make_move(&mut b_clone, player);
        let mut is_new = true;
        for permutation in b_clone.get_all_permutations::<true>(state.gods, state.base_hash()) {
            if unique_boards.contains(&permutation) {
                is_new = false;
                break;
            }
        }
        if is_new {
            unique_boards.push(b_clone.clone());
            res.push(p);
        }
    }

    res
}

pub fn get_placement_actions<const IS_UNIQUE: bool>(
    state: &FullGameState,
    placement_mode: PlacementState,
) -> Vec<WorkerPlacement> {
    let active_god = state.gods[placement_mode.next_placement as usize];
    if IS_UNIQUE {
        match active_god.num_workers {
            2 => get_unique_placements(&state, placement_mode.next_placement),
            3 => get_unique_placements_3(&state, placement_mode.next_placement),
            _ => unreachable!("Unknown worker count"),
        }
    } else {
        match active_god.num_workers {
            2 => get_all_placements(&state.board, placement_mode.next_placement),
            3 => get_all_placements_3(&state.board, placement_mode.next_placement),
            _ => unreachable!("Unknown worker count"),
        }
    }
}
