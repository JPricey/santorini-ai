use crate::{
    bitboard::{apply_mapping_to_mask, BitBoard, NEIGHBOR_MAP}, board::FullGameState, build_god_power_movers, gods::{
        build_god_power_actions, generic::{MoveGenFlags, ScoredMove}, god_power, mortal::MortalMove, move_helpers::{
            build_scored_move, get_basic_moves, get_generator_prelude_state, 
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        }, GodName, GodPower
    }, persephone_check_result, player::Player
};

pub(super) fn pan_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(pan_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2 | prelude.exactly_level_3;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = get_basic_moves::<MUST_CLIMB>(&prelude, &worker_start_state);

        if worker_start_state.worker_start_height == 2 {
            let winning_moves = worker_moves
                & (prelude.exactly_level_0 | prelude.exactly_level_3)
                & prelude.win_mask;

            if push_winning_moves::<F, MortalMove, _>(
                &mut result,
                worker_start_pos,
                winning_moves,
                MortalMove::new_winning_move,
            ) {
                return result;
            }

            worker_moves ^= winning_moves;
        } else if worker_start_state.worker_start_height == 3 {
            let winning_moves = worker_moves
                & (prelude.exactly_level_0 | prelude.exactly_level_1)
                & prelude.win_mask;
            if push_winning_moves::<F, MortalMove, _>(
                &mut result,
                worker_start_pos,
                winning_moves,
                MortalMove::new_winning_move,
            ) {
                return result;
            }

            worker_moves ^= winning_moves;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_lvl_2_workers = worker_start_state.other_own_workers & prelude.exactly_level_2;
        let neighbor_2 = apply_mapping_to_mask(other_lvl_2_workers, &NEIGHBOR_MAP);
        let neighbor_3 = apply_mapping_to_mask(
            worker_start_state.other_own_workers & prelude.exactly_level_3,
            &NEIGHBOR_MAP,
        );

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let mut reach_2 = neighbor_2;
            let mut reach_3 = neighbor_3;

            let next_turn_moves = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & worker_next_build_state.unblocked_squares;

            // dont worry about reach 3 vs hypnus because it's not possible to get up there
            if prelude.is_against_hypnus
                && (other_lvl_2_workers.count_ones() as u32 + worker_end_move_state.is_now_lvl_2)
                    < 2
            {
                reach_2 = BitBoard::EMPTY
            } else if worker_end_move_state.worker_end_height == 2 {
                reach_2 |= next_turn_moves;
            } else if worker_end_move_state.worker_end_height == 3 {
                reach_3 |= next_turn_moves;
            }

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let anti_worker_build_mask = !worker_build_mask;
                let winnable_from_2 = reach_2
                    & ((prelude.exactly_level_0 | prelude.exactly_level_3)
                        & anti_worker_build_mask
                        | prelude.exactly_level_2 & worker_build_mask);
                let winnable_from_3 = reach_3
                    & (prelude.exactly_level_0 | prelude.exactly_level_1 & anti_worker_build_mask);

                let check_board = (winnable_from_2 | winnable_from_3)
                    & worker_next_build_state.unblocked_squares
                    & !worker_end_move_state.worker_end_mask
                    & prelude.win_mask;
                let is_check = check_board.is_not_empty();

                let new_action = MortalMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
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

pub const fn build_pan() -> GodPower {
    god_power(
        GodName::Pan,
        build_god_power_movers!(pan_move_gen),
        build_god_power_actions::<MortalMove>(),
        9244705180822939865,
        18175931309899694692,
    )
}
