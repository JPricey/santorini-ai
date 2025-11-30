use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower,
        apollo::ApolloMove,
        build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        harpies::slide_position,
        move_helpers::{
            build_scored_move, get_basic_moves_from_raw_data_with_custom_blockers,
            get_generator_prelude_state, get_standard_reach_board_from_parts,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            is_stop_on_mate, modify_prelude_for_checking_workers,
        },
    },
    persephone_check_result,
    player::Player,
};

pub(super) fn apollo_v2_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(apollo_v2_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
            &prelude,
            worker_start_state.worker_start_pos,
            worker_start_state.worker_start_mask,
            worker_start_state.worker_start_height,
            worker_start_state.other_own_workers
                | prelude.domes_and_frozen
                | (prelude.oppo_workers
                    & prelude.board.height_map[worker_start_state.worker_start_height]),
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;

            for worker_end_pos in moves_to_level_3.into_iter() {
                let swap_square =
                    if (BitBoard::as_mask(worker_end_pos) & prelude.oppo_workers).is_empty() {
                        None
                    } else {
                        Some(worker_end_pos)
                    };
                let winning_move = ScoredMove::new_winning_move(
                    ApolloMove::new_apollo_winning_move(
                        worker_start_pos,
                        worker_end_pos,
                        swap_square,
                    )
                    .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }

            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &prelude.standard_neighbor_map);

        for mut worker_end_pos in worker_moves {
            let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);

            let is_swap = (BitBoard::as_mask(worker_end_pos) & prelude.oppo_workers).is_not_empty();
            let mut final_other_workers = prelude.oppo_workers;
            let mut final_build_mask = prelude.build_mask;
            let mut swap_square = None;

            let mut swap_mask = BitBoard::EMPTY;
            if is_swap {
                final_other_workers ^= worker_end_mask | worker_start_state.worker_start_mask;
                final_build_mask =
                    prelude.other_god.get_build_mask(final_other_workers) | prelude.exactly_level_3;
                swap_square = Some(worker_end_pos);
                swap_mask = BitBoard::as_mask(worker_end_pos);
            }

            if prelude.is_against_harpies {
                worker_end_pos = slide_position(&prelude, worker_start_pos, worker_end_pos);
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;
            let is_now_lvl_2 = (worker_end_height == 2) as u32;

            let self_blockers =
                prelude.domes_and_frozen | worker_start_state.other_own_workers | worker_end_mask;
            let unblocked_squares_for_builds = !(self_blockers | final_other_workers);

            let mut worker_builds = NEIGHBOR_MAP[worker_end_pos as usize]
                & unblocked_squares_for_builds
                & final_build_mask;

            if is_interact_with_key_squares::<F>() {
                if ((worker_start_state.worker_start_mask
                    & BitBoard::CONDITIONAL_MASK[is_swap as usize]
                    | worker_end_mask
                    | swap_mask)
                    & key_squares)
                    .is_empty()
                {
                    worker_builds &= key_squares;
                }
            }

            let reach_board = get_standard_reach_board_from_parts::<F>(
                &prelude,
                other_threatening_workers,
                other_threatening_neighbors,
                worker_end_pos,
                is_now_lvl_2,
                unblocked_squares_for_builds,
            );

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = ApolloMove::new_basic_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                    swap_square,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ))
            }
        }
    }

    result
}

pub(crate) const fn build_apollo_v2() -> GodPower {
    god_power(
        GodName::ApolloV2,
        build_god_power_movers!(apollo_v2_move_gen),
        build_god_power_actions::<ApolloMove>(),
        7217779490744502025,
        16422608020866574275,
    )
}
