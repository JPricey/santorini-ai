use crate::{
    bitboard::{BitBoard, apply_mapping_to_mask},
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        mortal::MortalMove,
        move_helpers::{
            GeneratorPreludeState, WorkerNextMoveState, WorkerStartMoveState, build_scored_move,
            get_generator_prelude_state, get_reach_board_when_can_be_level_3,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
            restrict_moves_by_affinity_area,
        },
    },
    persephone_check_result,
    player::Player,
};

fn _get_worker_moves<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    blocked_squares: BitBoard,
) -> BitBoard {
    if MUST_CLIMB {
        prelude.standard_neighbor_map[worker_start_state.worker_start_pos as usize]
            & prelude.board.height_map[worker_start_state.worker_start_height as usize]
            & !blocked_squares
    } else {
        if prelude.is_down_prevented {
            let down_mask =
                if prelude.is_down_prevented && worker_start_state.worker_start_height > 0 {
                    !prelude.board.height_map[worker_start_state.worker_start_height - 1]
                } else {
                    BitBoard::EMPTY
                };

            prelude.standard_neighbor_map[worker_start_state.worker_start_pos as usize]
                & !(down_mask | blocked_squares)
        } else if prelude.can_climb {
            prelude.standard_neighbor_map[worker_start_state.worker_start_pos as usize]
                & !blocked_squares
        } else {
            prelude.standard_neighbor_map[worker_start_state.worker_start_pos as usize]
                & !(prelude.board.height_map[worker_start_state.worker_start_height as usize]
                    | blocked_squares)
        }
    }
}

pub(super) fn pegasus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(pegasus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    modify_prelude_for_checking_workers::<F>(prelude.exactly_level_2, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let has_any_level_3_other_workers =
            (worker_start_state.other_own_workers & prelude.exactly_level_3).is_not_empty();

        let mut worker_next_moves = WorkerNextMoveState {
            other_threatening_workers: worker_start_state.other_own_workers
                & prelude.exactly_level_2,
            other_threatening_neighbors: apply_mapping_to_mask(
                worker_start_state.other_own_workers & prelude.exactly_level_2,
                prelude.standard_neighbor_map,
            ),

            worker_moves: {
                let base_worker_moves = _get_worker_moves::<MUST_CLIMB>(
                    &prelude,
                    &worker_start_state,
                    prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen,
                );
                restrict_moves_by_affinity_area(
                    worker_start_state.worker_start_mask,
                    base_worker_moves,
                    prelude.affinity_area,
                )
            },
        };

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
                    worker_end_move_state.is_now_lvl_2 > 0,
                ))
            }
        }
    }

    result
}

pub const fn build_pegasus() -> GodPower {
    god_power(
        GodName::Pegasus,
        build_god_power_movers!(pegasus_move_gen),
        build_god_power_actions::<MortalMove>(),
        16247440087819553927,
        7661264400958143927,
    )
}
