use crate::{
    bitboard::{apply_mapping_to_mask, BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP}, board::FullGameState, build_god_power_movers, gods::{
        build_god_power_actions, generic::{MoveGenFlags, ScoredMove}, god_power, mortal::MortalMove, move_helpers::{
            build_scored_move, get_generator_prelude_state, 
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        }, GodName, GodPower
    }, persephone_check_result, player::Player
};

pub(super) fn aphrodite_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(aphrodite_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
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

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes);

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;

            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let all_own_workers =
                    worker_start_state.other_own_workers | worker_end_move_state.worker_end_mask;
                let affinity_mask = apply_mapping_to_mask(all_own_workers, &INCLUSIVE_NEIGHBOR_MAP);

                let is_already_matched =
                    (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty()
                        || {
                            let affinity_match = affinity_mask & key_squares;
                            (affinity_match & prelude.oppo_workers).is_not_empty()
                                && affinity_match != key_squares
                        };
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched as usize];
            }

            for worker_build_pos in narrowed_builds {
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

pub const fn build_aphrodite() -> GodPower {
    god_power(
        GodName::Aphrodite,
        build_god_power_movers!(aphrodite_move_gen),
        build_god_power_actions::<MortalMove>(),
        2716361401804657736,
        2419339499129334119,
    )
    .with_is_aphrodite()
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::{fen::parse_fen, player::Player};

    #[test]
    fn test_aphrodite_affinity_check_blocking() {
        let state = parse_fen("00023 00000 00000 00000 00000/1/mortal:D5/aphrodite:B5").unwrap();

        let (mortal, aphro) = state.get_active_non_active_gods();

        let base_mortal_winning_moves = mortal.get_winning_moves(&state, Player::One);
        assert_eq!(base_mortal_winning_moves.len(), 1);

        let key_squares =
            mortal.get_blocker_board(&state.board, base_mortal_winning_moves[0].action);

        let aphro_moves = aphro.get_unscored_blocker_moves(&state, Player::Two, key_squares);
        // for m in &aphro_moves {
        //     eprintln!("{}", aphro.stringify_move(m.action));
        // }
        assert_eq!(aphro_moves.len(), 11);
    }

    #[test]
    fn test_aphrodite_affinity_not_wasteful_check_blocking() {
        let state = parse_fen("00032 00000 00000 00000 00000/1/mortal:E5/aphrodite:B5").unwrap();

        let (mortal, aphro) = state.get_active_non_active_gods();

        let base_mortal_winning_moves = mortal.get_winning_moves(&state, Player::One);
        assert_eq!(base_mortal_winning_moves.len(), 1);

        let key_squares =
            mortal.get_blocker_board(&state.board, base_mortal_winning_moves[0].action);

        let aphro_moves = aphro.get_unscored_blocker_moves(&state, Player::Two, key_squares);
        assert_eq!(aphro_moves.len(), 2);
    }
}
