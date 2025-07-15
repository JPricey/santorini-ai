use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        GodName, GodPower,
        generic::{
            CHECK_SENTINEL_SCORE, GENERATE_THREATS_ONLY, GRID_POSITION_SCORES, GenericMove,
            IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY,
            MoveGenFlags, NON_IMPROVER_SENTINEL_SCORE, RETURN_FIRST_MATE, STOP_ON_MATE, ScoredMove,
            WORKER_HEIGHT_SCORES,
        },
        mortal::{MortalMove, mortal_make_move, mortal_move_to_actions, mortal_unmake_move},
    },
    player::Player,
    utils::move_all_workers_one_include_original_workers,
};

fn artemis_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.at_least_level_1()
    }
    let capacity = if F & MATE_ONLY != 0 { 4 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_own_workers =
                (current_workers ^ moving_worker_start_mask) & board.exactly_level_2();
            for other_pos in other_own_workers {
                neighbor_check_if_builds |=
                    NEIGHBOR_MAP[other_pos as usize] & board.exactly_level_2();
            }
        }

        let mut first_worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[std::cmp::min(3, worker_starting_height + 1)] | all_workers_mask);

        let mut valid_second_moves = !first_worker_moves;

        if worker_starting_height == 2 {
            let moves_to_level_3 = first_worker_moves & board.height_map[2];
            first_worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    MortalMove::new_mortal_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                    )
                    .into(),
                );
                result.push(winning_move);
                if F & STOP_ON_MATE != 0 {
                    return result;
                }
            }
        }

        if F & MATE_ONLY != 0 {
            first_worker_moves &= board.exactly_level_2();
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for worker_mid_pos in first_worker_moves.into_iter() {
            let worker_mid_mask = BitBoard::as_mask(worker_mid_pos);
            let worker_mid_height = board.get_height_for_worker(worker_mid_mask);

            let mut worker_second_moves = NEIGHBOR_MAP[worker_mid_pos as usize]
                & valid_second_moves
                & !(board.height_map[std::cmp::min(3, worker_mid_height + 1)] | all_workers_mask);

            valid_second_moves ^= worker_second_moves;

            if worker_mid_height == 2 {
                let moves_to_level_3 = worker_second_moves & board.height_map[2];
                worker_second_moves ^= moves_to_level_3;

                for moving_worker_end_pos in moves_to_level_3.into_iter() {
                    let winning_move = ScoredMove::new_winning_move(
                        MortalMove::new_mortal_winning_move(
                            moving_worker_start_pos,
                            moving_worker_end_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if F & STOP_ON_MATE != 0 {
                        return result;
                    }
                }
            }

            if F & MATE_ONLY != 0 {
                continue;
            }

            for moving_worker_end_pos in worker_second_moves.into_iter() {
                let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
                let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

                let mut worker_builds = NEIGHBOR_MAP[worker_mid_pos as usize] & buildable_squares;
                if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                    if (moving_worker_end_mask & key_squares).is_empty() {
                        worker_builds = worker_builds & key_squares;
                    }
                }

                let mut check_if_builds = neighbor_check_if_builds;
                let mut anti_check_builds = BitBoard::EMPTY;
                let mut is_already_check = false;

                if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                    if worker_end_height == 2 {
                        check_if_builds |= worker_builds & board.exactly_level_2();
                        anti_check_builds =
                            NEIGHBOR_MAP[worker_mid_pos as usize] & board.exactly_level_3();
                        is_already_check = anti_check_builds != BitBoard::EMPTY;
                    }
                }

                if F & GENERATE_THREATS_ONLY != 0 {
                    if is_already_check {
                        let must_avoid_build = anti_check_builds & worker_builds;
                        if must_avoid_build.count_ones() == 1 {
                            worker_builds ^= must_avoid_build;
                        }
                    } else {
                        worker_builds &= check_if_builds;
                    }
                }

                for worker_build_pos in worker_builds {
                    let new_action = MortalMove::new_mortal_move(
                        moving_worker_start_pos,
                        worker_mid_pos,
                        worker_build_pos,
                    );
                    if F & INCLUDE_SCORE != 0 {
                        let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                        let score;
                        if is_already_check
                            && (anti_check_builds & !worker_build_mask).is_not_empty()
                            || (worker_build_mask & check_if_builds).is_not_empty()
                        {
                            score = CHECK_SENTINEL_SCORE;
                        } else {
                            let is_improving = worker_end_height > worker_starting_height;
                            score = if is_improving {
                                IMPROVER_SENTINEL_SCORE
                            } else {
                                NON_IMPROVER_SENTINEL_SCORE
                            };
                        }
                        result.push(ScoredMove::new(new_action.into(), score));
                    } else {
                        result.push(ScoredMove::new(new_action.into(), 0));
                    }
                }
            }
        }
    }

    result
}

pub const fn build_artemis() -> GodPower {
    GodPower {
        god_name: GodName::Artemis,
        get_all_moves: artemis_move_gen::<0>,
        get_moves: artemis_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        get_win: artemis_move_gen::<{ RETURN_FIRST_MATE }>,
        get_actions_for_move: mortal_move_to_actions,
        _make_move: mortal_make_move,
        _unmake_move: mortal_unmake_move,
    }
}

#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, gods::GodName, player::Player};

    // #[test]
    // fn test_artemis_basic() {
    //     let state = FullGameState::try_from("0000022222000000000000000/1/artemis:0,1/mortal:23,24")
    //         .unwrap();

    //     let next_states = state.get_next_states_interactive();
    //     // for state in next_states {
    //     //     state.state.print_to_console();
    //     //     println!("{:?}", state.actions);
    //     // }
    //     assert_eq!(next_states.len(), 10);
    // }

    // #[test]
    // fn test_artemis_cant_move_through_wins() {
    //     let state =
    //         FullGameState::try_from("2300044444000000000000000/1/artemis:0/mortal:24").unwrap();
    //     let next_states = state.get_next_states_interactive();
    //     assert_eq!(next_states.len(), 1);
    //     assert_eq!(next_states[0].state.board.get_winner(), Some(Player::One))
    // }

    // #[test]
    // fn test_artemis_win_check() {
    //     // Regular 1>2>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("12300 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         1
    //     );

    //     // Can't move 1>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("13300 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         0
    //     );

    //     // Can move 2>2>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("22300 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         1
    //     );

    //     // Can't move 2>1>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("21300 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         0
    //     );

    //     // Single move 2>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("23000 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         1
    //     );

    //     // Can't win from 3>3
    //     assert_eq!(
    //         (GodName::Artemis.to_power().get_win)(
    //             &FullGameState::try_from("33000 44444 44444 44444 44444/1/artemis:0/mortal:24")
    //                 .unwrap()
    //                 .board,
    //             Player::One
    //         )
    //         .len(),
    //         0
    //     );
    // }
}
