use arrayvec::ArrayVec;

use crate::{
    bitboard::{
        BitBoard, JUMP_OVER_PUSH_MAPPING, NEIGHBOR_MAP, NUM_SQUARES, PUSH_MAPPING,
        WRAPPING_NEIGHBOR_MAP,
    },
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
    let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
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

pub(crate) fn basic_slide_from_unblocked(
    prelude: &GeneratorPreludeState,
    unblocked_squares: BitBoard,
    from: Square,
    to: Square,
) -> (Square, usize) {
    let mut current_from = from;
    let mut current_pos = to;
    let mut current_height = prelude.board.get_height(current_pos);

    loop {
        let Some(next_spot) = PUSH_MAPPING[current_from as usize][current_pos as usize] else {
            break;
        };
        let next_mask = next_spot.to_board();
        if (unblocked_squares & next_mask).is_empty() {
            break;
        }

        let next_height = prelude.board.get_height(next_spot);
        if next_height > current_height {
            break;
        }

        current_from = current_pos;
        current_pos = next_spot;
        current_height = next_height;
    }

    return (current_pos, current_height);
}

#[derive(Copy, Clone)]
pub(crate) struct HarpyWorkerSlideState {
    /// The square the worker steps onto before the push takes over.
    pub dir: Square,
    pub dest: Square,
    pub height: usize,
    /// Every square the worker touches, start and dest included.
    pub cover: BitBoard,
}

pub(crate) fn harpy_slide_with_coverage(
    prelude: &GeneratorPreludeState,
    unblocked_squares: BitBoard,
    mut from: Square,
    mut to: Square,
) -> HarpyWorkerSlideState {
    let dir = to;
    let mut current_height = prelude.board.get_height(to);
    let mut cover = from.to_board() | to.to_board();

    loop {
        let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
            break;
        };

        let next_mask = next_spot.to_board();
        if (unblocked_squares & next_mask).is_empty() {
            break;
        }

        let new_height = prelude.board.get_height(next_spot);
        if new_height > current_height {
            // Can't climb
            break;
        }

        cover |= next_mask;

        from = to;
        to = next_spot;
        current_height = new_height;
    }

    HarpyWorkerSlideState {
        dir,
        dest: to,
        cover,
        height: current_height,
    }
}

/// First steps of a harpies-pushed move, for gods that move both workers in one turn.
/// Winning climbs are excluded, but a worker already on level 3 may still walk across to another.
pub(crate) fn get_harpy_double_move_first_steps(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    worker_start_pos: Square,
    worker_start_height: usize,
) -> BitBoard {
    let max_height = if worker_start_height == 3 {
        3
    } else {
        2.min(worker_start_height + 1)
    };

    NEIGHBOR_MAP[worker_start_pos as usize]
        & unblocked_non_own_workers
        & !prelude.board.height_map[max_height]
}

fn _resolve_harpy_double_move_with_first_mover<Emit: FnMut(Square, usize, Square, usize)>(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    first_start: Square,
    second_start: Square,
    first_mask: BitBoard,
    second_mask: BitBoard,
    first: &HarpyWorkerSlideState,
    second: &HarpyWorkerSlideState,
    is_swapped: bool,
    emit: &mut Emit,
) -> bool {
    // `first` moves first when:
    // - first's start is in second's way, and we want first to move out of the way first
    // - first's end is in second's way, and we want to see how that collision works
    // - second's start is in first's way

    let mut push = |first_dest: Square, first_height: usize, second_dest, second_height| {
        if is_swapped {
            emit(second_dest, second_height, first_dest, first_height);
        } else {
            emit(first_dest, first_height, second_dest, second_height);
        }
    };

    if (first.cover & second_mask).is_not_empty() {
        // Second starts in first's way
        if first.dir == second_start {
            // can't move first first at all, skip, but record that there was an interaction
            return true;
        }

        let (first_dest, first_dest_height) = basic_slide_from_unblocked(
            prelude,
            unblocked_non_own_workers & !second_mask,
            first_start,
            first.dir,
        );

        // First moved and now blocks second. Skip, but record that there was an interaction
        if first_dest == second.dir {
            return true;
        }

        // Second moves as if nothing was in the way now
        push(first_dest, first_dest_height, second.dest, second.height);
        return true;
    }

    let first_end_in_the_way = (second.cover & first.dest.to_board()).is_not_empty();
    if first_end_in_the_way {
        // First blocks second entirely
        if first.dest == second.dir {
            return true;
        }

        let (second_dest, second_dest_height) = basic_slide_from_unblocked(
            prelude,
            unblocked_non_own_workers & !first.dest.to_board(),
            second_start,
            second.dir,
        );

        push(first.dest, first.height, second_dest, second_dest_height);
        return true;
    }

    let first_start_in_the_way = (second.cover & first_mask).is_not_empty();
    if first_start_in_the_way {
        // First moves out of second's way to start
        // But we know that first doesn't end in second's way
        push(first.dest, first.height, second.dest, second.height);
    }

    false
}

/// Resolves where two workers end up when a god moves both in one turn and harpies pushes each
/// along. `emit` is called per legal ordering with `(w1_dest, w1_height, w2_dest, w2_height)`.
pub(crate) fn for_each_harpy_double_move<Emit: FnMut(Square, usize, Square, usize)>(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    worker_start_1: Square,
    worker_start_2: Square,
    all_moves_1: BitBoard,
    all_moves_2: BitBoard,
    mut emit: Emit,
) {
    let w1_mask = worker_start_1.to_board();
    let w2_mask = worker_start_2.to_board();

    let mut w1_moves: ArrayVec<HarpyWorkerSlideState, 8> = Default::default();
    for dir in all_moves_1 {
        w1_moves.push(harpy_slide_with_coverage(
            prelude,
            unblocked_non_own_workers,
            worker_start_1,
            dir,
        ));
    }

    for dir2 in all_moves_2 {
        let w2_move =
            harpy_slide_with_coverage(prelude, unblocked_non_own_workers, worker_start_2, dir2);

        // Let's try to break down the cases....
        // 1. w1 goes first then w2
        //  - maybe because w1 hits w2 before it moves (w2 start in w1 coverage)
        //  - maybe because w2 hits w1 after it moves (w1 end in w2 coverage)
        //  - but never both
        // 2. w2 goes first then w1
        //  - maybe because w2 hits w1 before it moves (w1 start in w2 coverage)
        //  - maybe because w1 hits w2 after it moves (w2 end in w1 coverage)
        //  - but never both
        // 3. if we haven't emit a move yet, it's because they are independant, so we can safely
        //    emit them both
        // 2/3 are weird because maybe you want to go first to "get there" first, or maybe you want
        // to go first to collide with the other worker first
        //
        // recalculate first if coverage line hits other worker start
        // recalculate second if coverage line hits other worker dest
        // force order when dir == other worker start or dest

        for w1_move in &w1_moves {
            let did_move_w1_first = _resolve_harpy_double_move_with_first_mover(
                prelude,
                unblocked_non_own_workers,
                worker_start_1,
                worker_start_2,
                w1_mask,
                w2_mask,
                w1_move,
                &w2_move,
                false,
                &mut emit,
            );

            let did_move_w2_first = _resolve_harpy_double_move_with_first_mover(
                prelude,
                unblocked_non_own_workers,
                worker_start_2,
                worker_start_1,
                w2_mask,
                w1_mask,
                &w2_move,
                w1_move,
                true,
                &mut emit,
            );

            if !did_move_w1_first && !did_move_w2_first {
                // independent moves
                emit(w1_move.dest, w1_move.height, w2_move.dest, w2_move.height);
            }
        }
    }
}

pub(crate) fn iris_slide_position(
    prelude: &GeneratorPreludeState,
    from: Square,
    to: Square,
) -> Square {
    let Some(next_spot) = JUMP_OVER_PUSH_MAPPING[from as usize][to as usize] else {
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
    let other_god = state.get_god_for_player(!player).god_name;
    // Block detection vs multi-step movers (artemis, stymphalians) can get pretty complicated...
    // just try every move.
    // Chronus can win on his build, not his move. Instead of checking for ways to block his moves,
    // just try everything too.
    let final_key_squares = if other_god == GodName::Artemis
        || other_god.is_chronus_variant()
        || other_god == GodName::Stymphalians
        // Harpies' slides can carry a Triton chain past a non-perimeter square, which is the one
        // case his blocker board cannot see from the board alone.
        || other_god == GodName::Triton
    {
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
            if god_name == GodName::Castor {
                continue;
            }
            // Jason's power places a new worker on the perimeter that then slides; in this
            // setup it can legitimately land at a neighbor of A5 (blocked by the existing
            // worker), so the "no neighbor" check doesn't apply to him.
            if god_name == GodName::Jason {
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
