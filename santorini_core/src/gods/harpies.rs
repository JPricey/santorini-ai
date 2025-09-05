use crate::{
    add_scored_move, after_move_power_generator,
    bitboard::{BitBoard, NUM_SQUARES, PUSH_MAPPING, WRAPPING_NEIGHBOR_MAP},
    board::BoardState,
    build_building_masks, build_god_power_movers,
    gods::{GodName, GodPower, build_god_power_actions, god_power, mortal::MortalMove},
    square::Square,
};

use const_for::const_for;

const fn mod_number_between_plus_minus_one(mut x: i32) -> i32 {
    while x > 1 {
        x -= 5;
    }
    while x < -1 {
        x += 5;
    }

    debug_assert!(x <= 1);
    debug_assert!(x >= -1);
    x
}

pub const MAY_WRAP_FROM_PUSH_MAPPING: [[Option<Square>; NUM_SQUARES]; NUM_SQUARES] = {
    let mut result = [[None; NUM_SQUARES]; NUM_SQUARES];
    const_for!(from in 0..25 => {
        let from_square = Square::const_from_u8(from);
        const_for!(to in 0..25 => {
            let to_square = Square::const_from_u8(to);
            let to_mask = BitBoard::as_mask(to_square);

            if (WRAPPING_NEIGHBOR_MAP[from as usize].bit_and(to_mask)).is_empty() {
                continue;
            }

            let from_i = from_square.to_icoord();
            let to_i = to_square.to_icoord();

            let mut delta = to_i.sub(from_i);
            delta.row = mod_number_between_plus_minus_one(delta.row);
            delta.col = mod_number_between_plus_minus_one(delta.col);

            let dest = to_i.add(delta);

            result[from as usize][to as usize] = dest.to_square()
        });
    });
    result
};

pub fn urania_slide(board: &BoardState, from: Square, to: Square, workers: BitBoard) -> Square {
    let Some(next_spot) = MAY_WRAP_FROM_PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let to_height = board.get_height(to);
    let next_height = board.get_height(next_spot);

    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if (workers & next_mask).is_not_empty() {
        return to;
    }

    slide_position_with_custom_worker_blocker(board, to, next_spot, workers)
}

pub fn prometheus_slide(board: &BoardState, from: Square, to: Square, to_height: usize) -> Square {
    let Some(next_spot) = MAY_WRAP_FROM_PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let next_height = board.get_height(next_spot);
    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if ((board.workers[0] | board.workers[1]) & next_mask).is_not_empty() {
        return to;
    }

    slide_position(board, to, next_spot)
}

pub fn slide_position(board: &BoardState, from: Square, to: Square) -> Square {
    let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let to_height = board.get_height(to);
    let next_height = board.get_height(next_spot);

    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if ((board.workers[0] | board.workers[1]) & next_mask).is_not_empty() {
        return to;
    }

    slide_position(board, to, next_spot)
}

pub fn slide_position_with_custom_worker_blocker(
    board: &BoardState,
    from: Square,
    to: Square,
    workers: BitBoard,
) -> Square {
    let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let to_height = board.get_height(to);
    let next_height = board.get_height(next_spot);

    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if (workers & next_mask).is_not_empty() {
        return to;
    }

    slide_position_with_custom_worker_blocker(board, to, next_spot, workers)
}

// Same as mortal, except for custom key moves logic vs artemis
after_move_power_generator!(
    harpies_move_gen,
    build_winning_move: MortalMove::new_winning_move,
    state: state,
    player: player,
    board: board,
    is_include_score: is_include_score,
    is_interact_with_key_squares: is_interact_with_key_squares,
    key_squares: key_squares,
    is_against_hypnus: is_against_hypnus,
    is_against_harpies: is_against_harpies,
    is_check: is_check,
    is_improving: is_improving,
    exactly_level_1: exactly_level_1,
    exactly_level_2: exactly_level_2,
    exactly_level_3: exactly_level_3,
    domes: domes,
    win_mask: win_mask,
    build_mask: build_mask,
    worker_start_pos: worker_start_pos,
    worker_start_mask: worker_start_mask,
    worker_end_pos: worker_end_pos,
    worker_end_mask: worker_end_mask,
    worker_end_height: worker_end_height,
    non_moving_workers: non_moving_workers,
    all_possible_builds: all_possible_builds,
    narrowed_builds: narrowed_builds,
    reach_board: reach_board,
    unblocked_squares: unblocked_squares,
    other_threatening_workers: other_threatening_workers,
    other_threatening_neighbors: other_threatening_neighbors,
    is_now_lvl_2: is_now_lvl_2,
    result: result,
    extra_init: let final_key_squares = if state.gods[!player as usize].god_name == GodName::Artemis {
        // artemis can do some crazy stuff... just try everything
        BitBoard::MAIN_SECTION_MASK
    } else {
        key_squares
    },
    move_block: {
        let unblocked_squares = !(non_moving_workers | worker_end_mask | domes);

        build_building_masks!(
            worker_end_pos: worker_end_pos,
            open_squares: unblocked_squares,
            build_mask: build_mask,
            is_interact_with_key_squares: is_interact_with_key_squares,
            key_squares_expr: ((worker_end_mask | worker_start_mask) & final_key_squares).is_empty(),
            key_squares: final_key_squares,

            all_possible_builds: all_possible_builds,
            narrowed_builds: narrowed_builds,
            worker_plausible_next_moves: worker_plausible_next_moves,
        );

        let reach_board = if is_against_hypnus
            && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
        {
            BitBoard::EMPTY
        } else {
            (other_threatening_neighbors
                | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                & win_mask
                & unblocked_squares
        };

        for worker_build_pos in narrowed_builds {
            let worker_build_mask = BitBoard::as_mask(worker_build_pos);
            let new_action = MortalMove::new_basic_move(
                worker_start_pos,
                worker_end_pos,
                worker_build_pos,
            );

            let is_check = {
                let final_level_3 = (exactly_level_2 & worker_build_mask)
                    | (exactly_level_3 & !worker_build_mask);
                let check_board =
                    reach_board & final_level_3;
                check_board.is_not_empty()
            };

            add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
        }
    }
);

pub const fn build_harpies() -> GodPower {
    god_power(
        GodName::Harpies,
        build_god_power_movers!(harpies_move_gen),
        build_god_power_actions::<MortalMove>(),
        10276148328807193798,
        10430305106761659855,
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        bitboard::NEIGHBOR_MAP,
        fen::parse_fen,
        gods::ALL_GODS_BY_ID,
        matchup::{Matchup, is_matchup_banned},
    };

    use super::*;

    #[test]
    fn test_all_gods_respect_harpies() {
        // Doesn't test for perfect correctness, but at least that they aren't doing their basic
        // moves

        for god in ALL_GODS_BY_ID {
            let god_name = god.god_name;
            let matchup = Matchup::new(god_name, GodName::Harpies);
            if is_matchup_banned(&matchup) {
                eprintln!("skipping banned matchup: {}", matchup);
                continue;
            }

            let state = parse_fen(&format!(
                "00000 00000 00000 00000 00000/1/{}:A5/harpies:E2,D1",
                god_name
            ))
            .unwrap();

            let next_states = god.get_all_next_states(&state);
            let old_neighbors = NEIGHBOR_MAP[Square::A5 as usize];

            for next_state in next_states {
                let new_workers = next_state.workers[0];
                if (old_neighbors & new_workers).is_not_empty() {
                    eprintln!("{:?}", state);
                    next_state.print_to_console();
                    assert!(
                        false,
                        "Other god didn't respect harpies movement: {:?}",
                        god.god_name
                    );
                }
            }
        }
    }
}
