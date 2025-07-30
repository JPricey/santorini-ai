use crate::{
    bitboard::BitBoard, board::{BoardState, NEIGHBOR_MAP}, build_god_power, gods::{
        generic::{
            GenericMove, MoveGenFlags, ScoredMove, CHECK_SENTINEL_SCORE, GENERATE_THREATS_ONLY, IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, NON_IMPROVER_SENTINEL_SCORE, STOP_ON_MATE
        }, mortal::{
            mortal_make_move, mortal_move_to_actions, mortal_score_moves, mortal_stringify, mortal_unmake_move, MortalMove
        }, GodName, GodPower
    }, player::Player, utils::move_all_workers_one_include_original_workers
};

type GodMove = MortalMove;

fn artemis_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let enemy_workers = board.workers[1 - current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.at_least_level_1()
    }
    let capacity = if F & MATE_ONLY != 0 { 4 } else { 128 };
    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);
    let all_workers_mask = board.workers[0] | board.workers[1];

    let starting_exactly_level_2 = board.exactly_level_2();
    let starting_exactly_level_3 = board.exactly_level_3();

    let mut height_map_clone = board.height_map.clone();

    let can_worker_climb = board.get_worker_can_climb(player);

    for moving_worker_start_pos in current_workers.into_iter() {
        // if moving_worker_start_pos != Square::E5 {
        //     continue;
        // }
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);
        let other_own_workers =
            (current_workers ^ moving_worker_start_mask) & board.at_least_level_1();

        let mut valid_destinations = !all_workers_mask & !board.at_least_level_4();

        let mut worker_1d_moves = (NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
            | moving_worker_start_mask)
            & valid_destinations;

        if worker_starting_height == 2 {
            let moves_to_level_3 = worker_1d_moves & starting_exactly_level_3;
            worker_1d_moves ^= moves_to_level_3;
            valid_destinations ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
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

        if can_worker_climb {
            let at_height_2_1d = worker_1d_moves & starting_exactly_level_2;
            let winning_moves_to_level_3 = move_all_workers_one_include_original_workers(at_height_2_1d)
                & starting_exactly_level_3
                & valid_destinations;

            valid_destinations ^= winning_moves_to_level_3;

            for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
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

        let worker_moves;
        if can_worker_climb {
            let level_0_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & board.exactly_level_0(),
            ) & !board.at_least_level_2();
            let level_1_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & board.exactly_level_1(),
            ) & !board.at_least_level_3();
            let level_23_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & board.at_least_level_2(),
            );
            worker_moves = valid_destinations & (level_0_moves | level_1_moves | level_23_moves);
        } else {
            let level_0_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & board.exactly_level_0(),
            ) & !board.at_least_level_1();
            let level_1_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & board.exactly_level_1(),
            ) & !board.at_least_level_2();
            let level_2_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & starting_exactly_level_2,
            ) & !board.at_least_level_3();
            let level_3_moves = move_all_workers_one_include_original_workers(
                worker_1d_moves & starting_exactly_level_3,
            );
            worker_moves = valid_destinations
                & (level_0_moves | level_1_moves | level_2_moves | level_3_moves);
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            // if moving_worker_end_pos != Square::C5 {
            //     continue;
            // }
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);

            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            for worker_build_pos in worker_builds {
                // if worker_build_pos != Square::D4 {
                //     continue;
                // }

                let mut is_check = false;
                if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                    let build_mask = BitBoard::as_mask(worker_build_pos);
                    let build_height = board.get_height_for_worker(build_mask);
                    height_map_clone[build_height] ^= build_mask;

                    let mut invalid_destinations = moving_worker_end_mask;

                    let mut moves_1d = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                        & !height_map_clone[std::cmp::min(3, worker_end_height + 1)]
                        | moving_worker_end_mask;
                    // println!("moves1d {moves_1d}");

                    // println!("other_workers {other_own_workers}");
                    for other_worker_pos in other_own_workers {
                        let other_worker_mask = BitBoard::as_mask(other_worker_pos);
                        let other_worker_height = board.get_height_for_worker(other_worker_mask);
                        // println!("other worker: {other_worker_height}");
                        let other_worker_contribution = NEIGHBOR_MAP[other_worker_pos as usize]
                            & !height_map_clone[std::cmp::min(3, other_worker_height + 1)]
                            | other_worker_mask;
                        moves_1d |= other_worker_contribution;

                        // println!("other other_worker_contribution: {other_worker_contribution}");

                        invalid_destinations |= other_worker_mask;
                    }
                    moves_1d &= !enemy_workers;

                    // println!("moves1d {moves_1d}");

                    let moves_1d_level_2 = moves_1d & (height_map_clone[1] & !height_map_clone[2]);
                    // println!("moves_1d_level_2 {moves_1d_level_2}");

                    let winning_moves =
                        move_all_workers_one_include_original_workers(moves_1d_level_2)
                            & height_map_clone[2]
                            & !(height_map_clone[3] | invalid_destinations);

                    // println!("winning moves: {winning_moves}");

                    is_check = winning_moves.is_not_empty();
                    height_map_clone[build_height] ^= build_mask;
                }

                if F & GENERATE_THREATS_ONLY != 0 && !is_check {
                    continue;
                }

                let new_action = GodMove::new_mortal_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );

                if F & INCLUDE_SCORE != 0 {
                    let score;
                    if is_check {
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

pub fn artemis_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    let from = action.move_from_position();
    let to = action.move_to_position();

    (NEIGHBOR_MAP[from as usize] & NEIGHBOR_MAP[to as usize])
        | BitBoard::as_mask(action.move_to_position())
}

build_god_power!(
    build_artemis,
    god_name: GodName::Artemis,
    move_gen: artemis_move_gen,
    actions: mortal_move_to_actions,
    score_moves: mortal_score_moves,
    blocker_board: artemis_blocker_board,
    make_move: mortal_make_move,
    unmake_move: mortal_unmake_move,
    stringify: mortal_stringify,
);

#[cfg(test)]
mod tests {
    use crate::{
        board::FullGameState,
        gods::{
            GodName,
            artemis::{self, GodMove},
            generic::CHECK_SENTINEL_SCORE,
        },
        player::Player,
        random_utils::GameStateFuzzer,
    };

    #[test]
    fn test_artemis_basic() {
        let state = FullGameState::try_from("0000022222000000000000000/1/artemis:0,1/mortal:23,24")
            .unwrap();

        let next_states = state.get_next_states_interactive();
        // for state in &next_states {
        //     state.state.print_to_console();
        //     println!("{:?}", state.actions);
        // }
        assert_eq!(next_states.len(), 10);
    }

    #[test]
    fn test_artemis_cant_move_through_wins() {
        let state =
            FullGameState::try_from("2300044444000000000000000/1/artemis:0/mortal:24").unwrap();
        let next_states = state.get_next_states_interactive();
        // for state in &next_states {
        //     state.state.print_to_console();
        //     println!("{:?}", state.actions);
        // }
        assert_eq!(next_states.len(), 1);
        assert_eq!(next_states[0].state.board.get_winner(), Some(Player::One))
    }

    #[test]
    fn test_artemis_win_check() {
        let artemis = GodName::Artemis.to_power();
        // Regular 1>2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("12300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            1
        );

        // Can't move 1>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("13300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            0
        );

        // Can move 2>2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("22300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            1
        );

        // Can't move 2>1>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("21300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            0
        );

        // Single move 2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("23000 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            1
        );

        // Can't win from 3>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from("33000 44444 44444 44444 44444/1/artemis:0/mortal:24")
                        .unwrap()
                        .board,
                    Player::One
                )
                .len(),
            0
        );
    }

    #[test]
    fn test_artemis_check_detection() {
        let artemis = GodName::Artemis.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = artemis.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = artemis.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                artemis.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    artemis.get_winning_moves(&board, current_player).len() > 0;

                if is_check_move != is_winning_next_turn {
                    println!(
                        "Failed check detection. Check guess: {:?}. Actual: {:?}",
                        is_check_move, is_winning_next_turn
                    );
                    println!("{:?}", state);
                    state.board.print_to_console();
                    let act: GodMove = action.action.into();
                    println!("{:?}", act);
                    board.print_to_console();
                    assert_eq!(is_check_move, is_winning_next_turn);
                }
            }
        }
    }

    #[test]
    fn test_artemis_improver_checks_only() {
        let artemis = GodName::Artemis.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = artemis.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = artemis.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    artemis.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action,);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = artemis.get_moves_for_search(&state.board, current_player);
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
    fn debug_artemis_check_detection() {
        let mortal = GodName::Artemis.to_power();
        let state =
            FullGameState::try_from("0200001120012000011000010/1/mortal:B4,E5/mortal:A4,D3")
                .unwrap();
        state.print_to_console();

        let actions = mortal.get_moves_for_search(&state.board, Player::One);
        println!("num actions: {}", actions.len());
        for action in actions {
            let a: GodMove = action.action.into();
            println!("{:?}, {}", a, action.score);
        }
    }
}
