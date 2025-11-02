use crate::{
    bitboard::{BitBoard, BitboardMapping, NEIGHBOR_MAP, NUM_SQUARES, apply_mapping_to_mask},
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        harpies::slide_position_with_custom_blockers,
        mortal::MortalMove,
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_basic_moves,
            get_basic_moves_from_raw_data, get_generator_prelude_state,
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            push_winning_moves,
        },
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
};

pub fn _proteus_single_worker_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
) -> Vec<ScoredMove> {
    let mut result = Vec::new();

    let worker_start_pos = prelude.own_workers.lsb();
    let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

    if is_mate_only::<F>() && worker_start_state.worker_start_height != 2 {
        return result;
    }

    let mut worker_moves = get_basic_moves::<MUST_CLIMB>(&prelude, &worker_start_state);

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
        return result;
    }

    for worker_end_pos in worker_moves {
        let worker_end_move_state =
            get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

        let blocked_squares = (prelude.all_workers_and_frozen_mask
            ^ worker_start_state.worker_start_mask)
            | worker_end_move_state.worker_end_mask
            | prelude.domes_and_frozen;
        let unblocked_squares = !blocked_squares;

        let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
            & unblocked_squares
            & prelude.build_mask;
        let mut narrowed_builds = all_possible_builds;
        if is_interact_with_key_squares::<F>() {
            let is_already_matched = (worker_end_move_state.worker_end_mask & prelude.key_squares)
                .is_not_empty() as usize;
            narrowed_builds &=
                [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
        }

        let next_turn_moves = if worker_end_move_state.worker_end_height == 2 {
            prelude.standard_neighbor_map[worker_end_pos as usize] & unblocked_squares
        } else {
            BitBoard::EMPTY
        };

        for worker_build_pos in narrowed_builds {
            let build_mask = worker_build_pos.to_board();
            let new_action = MortalMove::new_basic_move(
                worker_start_pos,
                worker_end_move_state.worker_end_pos,
                worker_build_pos,
            );

            let is_check = {
                let final_level_2 = (prelude.exactly_level_2 & build_mask)
                    | (prelude.exactly_level_3 & !build_mask);
                let check_board = next_turn_moves & final_level_2;
                check_board.is_not_empty()
            };

            result.push(build_scored_move::<F, _>(
                new_action,
                is_check,
                worker_end_move_state.is_improving,
            ));
        }
    }

    result
}

pub(super) fn proteus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(proteus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);
    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    if prelude.own_workers.count() == 1 {
        return _proteus_single_worker_move_gen::<F, MUST_CLIMB>(&prelude);
    }

    if is_mate_only::<F>() {
        for worker_start_pos in prelude.acting_workers & prelude.exactly_level_2 {
            let worker_start_mask = worker_start_pos.to_board();
            let worker_start_height = prelude.board.get_height(worker_start_pos);
            let worker_moves = get_basic_moves_from_raw_data::<MUST_CLIMB>(
                &prelude,
                worker_start_pos,
                worker_start_mask,
                worker_start_height,
            );
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MortalMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MortalMove::new_winning_move,
            ) {
                return result;
            }
        }

        return result;
    }

    let mut worker_moves_map: BitboardMapping = [BitBoard::EMPTY; NUM_SQUARES];
    for worker_start_pos in prelude.acting_workers {
        let worker_start_mask = worker_start_pos.to_board();
        let worker_start_height = prelude.board.get_height(worker_start_pos);
        let mut worker_moves = get_basic_moves_from_raw_data::<MUST_CLIMB>(
            &prelude,
            worker_start_pos,
            worker_start_mask,
            worker_start_height,
        );

        if worker_start_height == 2 {
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

        worker_moves_map[worker_start_pos as usize] = worker_moves;
    }

    for worker_start_pos in prelude.own_workers {
        let worker_start_mask = worker_start_pos.to_board();
        let original_worker_start_height = prelude.board.get_height(worker_start_pos);
        let anti_worker_start_mask = !worker_start_mask;
        let other_active_workers = prelude.acting_workers & anti_worker_start_mask;

        let other_threatening_workers =
            (prelude.own_workers ^ worker_start_mask) & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, prelude.standard_neighbor_map);

        let blockers_without_starting_worker =
            (prelude.all_workers_and_frozen_mask ^ worker_start_mask) | prelude.domes_and_frozen;
        let mut remaining_moves = BitBoard::MAIN_SECTION_MASK;

        for other_worker_start_pos in other_active_workers {
            let mut other_worker_moves = worker_moves_map[other_worker_start_pos as usize];

            if !prelude.is_against_harpies {
                other_worker_moves &= remaining_moves;
                remaining_moves ^= other_worker_moves;
            }

            for worker_move_dir in other_worker_moves {
                let worker_end_pos;
                let worker_end_mask;

                if prelude.is_against_harpies {
                    worker_end_pos = slide_position_with_custom_blockers(
                        prelude.board,
                        other_worker_start_pos,
                        worker_move_dir,
                        blockers_without_starting_worker,
                    );
                    worker_end_mask = worker_end_pos.to_board();

                    if (worker_end_mask & remaining_moves).is_empty() {
                        continue;
                    }
                    remaining_moves ^= worker_end_mask;
                } else {
                    worker_end_pos = worker_move_dir;
                    worker_end_mask = worker_end_pos.to_board();
                }

                let worker_end_height = prelude.board.get_height(worker_end_pos);
                let is_improving = worker_end_height > original_worker_start_height;

                let final_blockers = blockers_without_starting_worker | worker_end_mask;
                let final_open_squares = !final_blockers;

                let all_possible_builds =
                    NEIGHBOR_MAP[worker_end_pos as usize] & final_open_squares & prelude.build_mask;

                let mut narrowed_builds = all_possible_builds;
                if is_interact_with_key_squares::<F>() {
                    let is_already_matched =
                        (worker_end_mask & prelude.key_squares).is_not_empty() as usize;
                    narrowed_builds &=
                        [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
                }

                let reach_board = get_standard_reach_board_from_parts::<F>(
                    &prelude,
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_end_pos,
                    (worker_end_height == 2) as u32,
                    final_open_squares,
                );

                for worker_build_pos in narrowed_builds {
                    let build_mask = worker_build_pos.to_board();
                    let new_action = MortalMove::new_basic_move(
                        worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                    );

                    let is_check = {
                        let final_level_3 = (prelude.exactly_level_2 & build_mask)
                            | (prelude.exactly_level_3 & !build_mask);
                        let check_board = reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        is_improving,
                    ));
                }
            }
        }
    }

    result
}

pub const fn build_proteus() -> GodPower {
    god_power(
        GodName::Proteus,
        build_god_power_movers!(proteus_move_gen),
        build_god_power_actions::<MortalMove>(),
        11735363125997027301,
        16382114980006810069,
    )
    .with_nnue_god_name(GodName::Graeae)
    .with_placement_type(PlacementType::ThreeWorkers)
}
