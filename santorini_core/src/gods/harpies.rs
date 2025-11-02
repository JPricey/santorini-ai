use crate::{
    bitboard::{BitBoard, NUM_SQUARES, PUSH_MAPPING, WRAPPING_NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        mortal::MortalMove,
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
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

    slide_position_with_custom_blockers(board, to, next_spot, workers)
}

pub(crate) fn prometheus_slide(
    prelude: &GeneratorPreludeState,
    from: Square,
    to: Square,
    to_height: usize,
) -> Square {
    let Some(next_spot) = MAY_WRAP_FROM_PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let next_height = prelude.board.get_height(next_spot);
    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if ((prelude.all_workers_and_frozen_mask) & next_mask).is_not_empty() {
        return to;
    }

    slide_position(prelude, to, next_spot)
}

pub(crate) fn slide_position(prelude: &GeneratorPreludeState, from: Square, to: Square) -> Square {
    let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let to_height = prelude.board.get_height(to);
    let next_height = prelude.board.get_height(next_spot);

    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);
    if ((prelude.all_workers_and_frozen_mask) & next_mask).is_not_empty() {
        return to;
    }

    slide_position(prelude, to, next_spot)
}

pub fn slide_position_with_custom_blockers(
    board: &BoardState,
    from: Square,
    to: Square,
    blockers: BitBoard,
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
    if (blockers & next_mask).is_not_empty() {
        return to;
    }

    slide_position_with_custom_blockers(board, to, next_spot, blockers)
}

// Same as mortal, except for custom key moves logic vs artemis
pub fn harpies_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    // Block detection vs artemis can get pretty complicated... just try every move
    let final_key_squares = if state.gods[!player as usize].god_name == GodName::Artemis {
        BitBoard::MAIN_SECTION_MASK
    } else {
        key_squares
    };

    let mut result = persephone_check_result!(harpies_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);
    let mut prelude = get_generator_prelude_state::<F>(state, player, final_key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MortalMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MortalMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = MortalMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    result
}

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
