use crate::{
    bitboard::BitBoard,
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        mortal::MortalMove,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_reach_board_when_can_be_level_3,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
};

pub(super) fn zeus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(zeus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        let has_any_level_3_other_workers =
            (worker_start_state.other_own_workers & prelude.exactly_level_3).is_not_empty();

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

            let is_already_matched =
                (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty();
            if (!is_interact_with_key_squares::<F>() || is_already_matched)
                && worker_end_move_state.worker_end_height < 3
                && (prelude.build_mask & worker_end_move_state.worker_end_mask).is_not_empty()
            {
                // Zeus build
                let new_action = MortalMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.worker_end_pos,
                );

                let reach_board = get_reach_board_when_can_be_level_3::<F>(
                    &prelude,
                    &worker_next_moves,
                    has_any_level_3_other_workers,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.worker_end_height + 1,
                    worker_next_build_state.unblocked_squares,
                );

                let is_check = {
                    let check_board = reach_board & prelude.exactly_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.worker_end_height
                        >= worker_start_state.worker_start_height,
                ));
            }

            let reach_board = get_reach_board_when_can_be_level_3::<F>(
                &prelude,
                &worker_next_moves,
                has_any_level_3_other_workers,
                worker_end_move_state.worker_end_pos,
                worker_end_move_state.worker_end_height,
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

pub const fn build_zeus() -> GodPower {
    god_power(
        GodName::Zeus,
        build_god_power_movers!(zeus_move_gen),
        build_god_power_actions::<MortalMove>(),
        12061343469622292818,
        398941887106100521,
    )
}
