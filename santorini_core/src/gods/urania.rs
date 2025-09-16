use crate::{
    bitboard::{
        BitBoard, WIND_AWARE_WRAPPING_NEIGHBOR_MAP, WRAPPING_NEIGHBOR_MAP, apply_mapping_to_mask,
    },
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        harpies::urania_slide,
        mortal::MortalMove,
        move_helpers::{
            GeneratorPreludeState, WorkerStartMoveState, build_scored_move,
            get_generator_prelude_state, get_worker_climb_height, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
};

fn get_must_climb_worker_moves(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
) -> BitBoard {
    let height_mask = match worker_start_state.worker_start_height {
        0 => prelude.exactly_level_1,
        1 => prelude.exactly_level_2,
        2 => prelude.exactly_level_3,
        3 => return BitBoard::EMPTY,
        _ => unreachable!(),
    };

    WRAPPING_NEIGHBOR_MAP[worker_start_state.worker_start_pos as usize]
        & height_mask
        & !prelude.all_workers_and_frozen_mask
}

fn urania_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(urania_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    modify_prelude_for_checking_workers::<F>(prelude.exactly_level_2, &mut prelude);

    let mut null_build_blocker = BitBoard::MAIN_SECTION_MASK;
    let wind_aware_neighbors = &WIND_AWARE_WRAPPING_NEIGHBOR_MAP[prelude.wind_idx];

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = if MUST_CLIMB {
            get_must_climb_worker_moves(&prelude, &worker_start_state)
        } else {
            let down_mask =
                if prelude.is_down_prevented && worker_start_state.worker_start_height > 0 {
                    !prelude.board.height_map[worker_start_state.worker_start_height - 1]
                } else {
                    BitBoard::EMPTY
                };

            let climb_height = get_worker_climb_height(&prelude, &worker_start_state);
            wind_aware_neighbors[worker_start_pos as usize]
                & !(prelude.board.height_map[climb_height] | down_mask | prelude.all_workers_and_frozen_mask)
        };

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

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &wind_aware_neighbors);

        let buildable_squares = !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);
        let mut already_seen = BitBoard::EMPTY;

        for mut worker_end_pos in worker_moves {
            let worker_end_mask;
            if prelude.is_against_harpies {
                worker_end_pos = urania_slide(
                    &prelude.board,
                    worker_start_pos,
                    worker_end_pos,
                    worker_start_state.all_non_moving_workers,
                );
                worker_end_mask = BitBoard::as_mask(worker_end_pos);

                if (worker_end_mask & already_seen).is_not_empty() {
                    continue;
                }
                already_seen |= worker_end_mask;
            } else {
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;
            let not_own_workers = !(worker_start_state.other_own_workers | worker_end_mask);
            let is_now_lvl_2 = (worker_end_height == 2) as usize;

            let mut worker_builds =
                WRAPPING_NEIGHBOR_MAP[worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = wind_aware_neighbors[worker_end_pos as usize] & buildable_squares;
            worker_builds &= prelude.build_mask;

            if is_interact_with_key_squares::<F>() {
                if (worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            if is_stop_on_mate::<F>() && worker_end_pos == worker_start_pos {
                worker_builds &= null_build_blocker;
                null_build_blocker ^= worker_builds;
            }

            let reach_board = if prelude.is_against_hypnus
                && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                    & prelude.win_mask
                    & buildable_squares
            };

            for worker_build_pos in worker_builds {
                let new_action =
                    MortalMove::new_basic_move(worker_start_pos, worker_end_pos, worker_build_pos);

                let is_check = {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = ((prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask))
                        & not_own_workers;
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

pub const fn build_urania() -> GodPower {
    god_power(
        GodName::Urania,
        build_god_power_movers!(urania_move_gen),
        build_god_power_actions::<MortalMove>(),
        9064977946056493903,
        14574722042933820831,
    )
}
