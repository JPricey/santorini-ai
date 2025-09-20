use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::FullGameState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        mortal::{MortalMove, mortal_move_gen},
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
};

fn persephone_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(mortal_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let vs_pan_key_builds = if prelude.other_god.god_name == GodName::Pan {
        apply_mapping_to_mask(prelude.oppo_workers, &NEIGHBOR_MAP)
    } else {
        BitBoard::EMPTY
    };

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
                | prelude.domes_and_frozen);
            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares
                    | vs_pan_key_builds & worker_start_state.worker_start_mask)
                    .is_not_empty() as usize;
                narrowed_builds &= [
                    prelude.key_squares | vs_pan_key_builds,
                    BitBoard::MAIN_SECTION_MASK,
                ][is_already_matched];
            }

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

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

pub const fn build_persephone() -> GodPower {
    god_power(
        GodName::Persephone,
        build_god_power_movers!(persephone_move_gen),
        build_god_power_actions::<MortalMove>(),
        14142160666731608851,
        6544769205610216454,
    )
    .with_is_persephone()
}

#[cfg(test)]
mod tests {
    use crate::{
        consistency_checker::ConsistencyChecker,
        fen::parse_fen,
        search::{SearchContext, WINNING_SCORE_BUFFER, negamax_search},
        search_terminators::DynamicMaxDepthSearchTerminator,
        transposition_table::TranspositionTable,
    };

    #[test]
    fn test_persephone_pan_blocker() {
        let state = parse_fen("0000200000300000200001000/1/persephone:A1,B1/pan:E5,C1").unwrap();

        let mut checker = ConsistencyChecker::new(&state);
        checker.perform_all_validations().expect("Failed check");
    }

    #[test]
    fn test_full_playout_persephone_pan_blocker() {
        let state = parse_fen("0000200000300000200001000/1/persephone:A1,B1/pan:E5,C1").unwrap();

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(2),
        };
        let search_state = negamax_search(&mut search_context, state);
        // Persephone is winning from here
        assert!(search_state.best_move.unwrap().score > WINNING_SCORE_BUFFER);
    }
}
