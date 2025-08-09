use crate::{
    bitboard::BitBoard, board::{BoardState, NEIGHBOR_MAP}, build_god_power, gods::{
        generic::{
            MoveGenFlags, ScoredMove, CHECK_SENTINEL_SCORE, GENERATE_THREATS_ONLY, IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, NON_IMPROVER_SENTINEL_SCORE, STOP_ON_MATE
        }, mortal::{
            mortal_blocker_board, mortal_make_move, mortal_move_to_actions, mortal_score_moves, mortal_stringify, mortal_unmake_move, MortalMove
        }, GodName, GodPower
    }, player::Player
};

type GodMove = MortalMove;

fn pan_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let other_workers = board.workers[1 - current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.at_least_level_2();
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    let level_2_winning_destinations = board.at_least_level_3() | board.exactly_level_0();
    let level_3_winning_destinations = !board.at_least_level_2();

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        let mut check_if_avoid_builds_and_moves = BitBoard::EMPTY;

        if F & INCLUDE_SCORE != 0 {
            let other_own_level_2_workers =
                (current_workers ^ moving_worker_start_mask) & board.exactly_level_2();
            for other_pos in other_own_level_2_workers {
                let other_neighbors = NEIGHBOR_MAP[other_pos as usize];
                neighbor_check_if_builds |= other_neighbors & board.exactly_level_2();
                check_if_avoid_builds_and_moves |=
                    other_neighbors & board.exactly_level_0() & !other_workers;
            }

            let other_own_level_3_workers =
                (current_workers ^ moving_worker_start_mask) & board.exactly_level_3();

            for other_pos in other_own_level_3_workers {
                let other_neighbors = NEIGHBOR_MAP[other_pos as usize];
                check_if_avoid_builds_and_moves |=
                    other_neighbors & !board.at_least_level_2() & !other_workers;
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if worker_starting_height == 2 {
            let winning_moves = worker_moves & level_2_winning_destinations;
            worker_moves ^= winning_moves;

            for moving_worker_end_pos in winning_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_mortal_winning_move(
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
        } else if worker_starting_height == 3 {
            let winning_moves = worker_moves & level_3_winning_destinations;
            worker_moves ^= winning_moves;

            for moving_worker_end_pos in winning_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_mortal_winning_move(
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

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let mut check_if_avoid_builds =
                check_if_avoid_builds_and_moves & !moving_worker_end_mask;

            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            // println!("original is already check: {}", is_already_check);

            let mut check_if_builds = neighbor_check_if_builds;
            let mut check_no_matter_what_build = false;

            if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & board.exactly_level_2();
                    check_if_avoid_builds |=
                        worker_builds & (board.exactly_level_3() | board.exactly_level_0());

                    // println!("{moving_worker_end_pos}");
                    // println!("check if: {}", check_if_builds);
                    // println!("anti checks: {}", check_if_avoid_builds);
                } else if worker_end_height == 3 {
                    check_if_avoid_builds |= worker_builds & board.exactly_level_1();
                    check_no_matter_what_build =
                        (worker_builds & board.exactly_level_0()).is_not_empty();
                }
            }

            if F & GENERATE_THREATS_ONLY != 0 {
                if check_no_matter_what_build {
                    // noop
                } else if check_if_avoid_builds.is_not_empty() {
                    let must_avoid_build = check_if_avoid_builds & worker_builds;
                    if must_avoid_build.count_ones() == 1 {
                        worker_builds ^= must_avoid_build;
                    }
                } else {
                    worker_builds &= check_if_builds;
                }
            }

            for worker_build_pos in worker_builds {
                let new_action = GodMove::new_mortal_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let score;
                    if check_no_matter_what_build
                        || (check_if_avoid_builds & !worker_build_mask).is_not_empty()
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

    result
}

build_god_power!(
    build_pan,
    god_name: GodName::Pan,
    move_gen: pan_move_gen,
    actions: mortal_move_to_actions,
    score_moves: mortal_score_moves,
    blocker_board: mortal_blocker_board,
    make_move: mortal_make_move,
    unmake_move: mortal_unmake_move,
    stringify: mortal_stringify,
);

#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, random_utils::GameStateFuzzer};

    use super::*;

    #[test]
    fn test_pan_check_detection() {
        let god = GodName::Pan.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = god.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = god.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                god.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn = god.get_winning_moves(&board, current_player).len() > 0;

                if is_check_move != is_winning_next_turn {
                    println!(
                        "Failed check detection. Check guess: {:?}. Actual: {:?}",
                        is_check_move, is_winning_next_turn
                    );
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(is_check_move, is_winning_next_turn);
                }
            }
        }
    }

    #[test]
    fn test_pan_improver_checks_only() {
        let pan = GodName::Pan.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = pan.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = pan.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    pan.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action,);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = pan.get_moves_for_search(&state.board, current_player);
            let check_count = all_moves
                .iter()
                .filter(|a| a.score == CHECK_SENTINEL_SCORE)
                .count();

            if improver_moves.len() != check_count {
                println!("Move count mismatch");
                state.board.print_to_console();
                println!("{:?}", state);

                improver_moves.sort_by_key(|a| -a.score);
                all_moves.sort_by_key(|a| -a.score);

                println!("IMPROVERS:");
                for a in &improver_moves {
                    println!("{:?}", a);
                }
                println!("ALL:");
                for a in &all_moves {
                    println!("{:?}", a);
                }

                assert_eq!(improver_moves.len(), check_count);
            }
        }
    }

    #[test]
    fn pan_test_check_detection_example() {
        let pan = GodName::Pan.to_power();
        // let state_str = "12000 44444 00000 00000 00000/1/mortal:A5/mortal:E1,E2";
        // let state_str = "4211002402201302121000020/1/mortal:B2,B3/mortal:B4,E2";
        let state_str = "00210 04444 44444 44444 00000/1/mortal:B5,C5/mortal:A1,B1";
        let state = FullGameState::try_from(state_str).unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = pan.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            let a: GodMove = action.action.into();
            println!("{:?}, {}", a, action.score);
        }
    }
}
