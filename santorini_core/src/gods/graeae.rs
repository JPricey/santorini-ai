use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        mortal::MortalMove,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_sized_result,
            get_worker_end_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    player::Player,
};

pub fn graeae_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut other_threatening_neighbors = BitBoard::EMPTY;
        let mut other_all_neighbors = BitBoard::EMPTY;

        for other_pos in worker_start_state.other_own_workers {
            other_all_neighbors |= NEIGHBOR_MAP[other_pos as usize];
            if prelude.board.get_height(other_pos) == 2 {
                other_threatening_neighbors |= NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[worker_start_pos as usize]
            & !(prelude.board.height_map[prelude
                .board
                .get_worker_climb_height(player, worker_start_state.worker_start_height)]
                | prelude.all_workers_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MortalMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MortalMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let open_squares = !(worker_start_state.all_non_moving_workers
                | prelude.domes
                | worker_end_move_state.worker_end_mask);
            let mut worker_builds = other_all_neighbors & open_squares;
            let worker_plausible_next_moves =
                NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize] & open_squares;
            worker_builds &= prelude.build_mask;

            if is_interact_with_key_squares::<F>() {
                if (worker_end_move_state.worker_end_mask & key_squares).is_empty() {
                    worker_builds &= key_squares;
                }
            }

            let reach_board = if prelude.is_against_hypnus
                && ((worker_start_state.other_own_workers & prelude.exactly_level_2).count_ones()
                    + worker_end_move_state.is_now_lvl_2)
                    < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (worker_plausible_next_moves
                        & BitBoard::CONDITIONAL_MASK[worker_end_move_state.is_now_lvl_2 as usize]))
                    & prelude.win_mask
                    & open_squares
            };

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = MortalMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
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
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    result
}

pub const fn build_graeae() -> GodPower {
    god_power(
        GodName::Graeae,
        build_god_power_movers!(graeae_move_gen),
        build_god_power_actions::<MortalMove>(),
        3621759432554562343,
        8641066751388211347,
    )
    .with_num_workers(3)
}
