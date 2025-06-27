use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        GodName, GodPower,
        generic::{
            GRID_POSITION_SCORES, GenericMove, INCLUDE_SCORE, MATE_ONLY, MoveGenFlags,
            RETURN_FIRST_MATE, STOP_ON_MATE, WORKER_HEIGHT_SCORES,
        },
        mortal::{mortal_make_move, mortal_move_to_actions, mortal_unmake_move},
    },
    player::Player,
    utils::move_all_workers_one_include_original_workers,
};

fn artemis_move_gen<const F: MoveGenFlags>(board: &BoardState, player: Player) -> Vec<GenericMove> {
    let mut result = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let current_workers = starting_current_workers;

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let mut already_counted_as_wins_mask = BitBoard::EMPTY;

        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let baseline_score = 50
            - GRID_POSITION_SCORES[moving_worker_start_pos as usize]
            - WORKER_HEIGHT_SCORES[worker_starting_height];

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_first_degree_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        if worker_starting_height == 2 {
            let moves_to_level_3 = worker_first_degree_moves & board.height_map[2];
            already_counted_as_wins_mask = moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = GenericMove::new_mortal_winning_move(
                    moving_worker_start_mask,
                    BitBoard::as_mask(moving_worker_end_pos),
                );
                result.push(winning_move);
                if F & STOP_ON_MATE != 0 {
                    return result;
                }
            }

            worker_first_degree_moves ^= moves_to_level_3;
        }

        let moves_from_level_2 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & board.height_map[1] & !board.height_map[2],
        ) & !board.height_map[3];

        let exactly_level_3 = board.height_map[2] & !board.height_map[3];
        let moves_from_2_to_3 = moves_from_level_2
            & exactly_level_3
            & !(already_counted_as_wins_mask | all_workers_mask);

        already_counted_as_wins_mask |= moves_from_2_to_3;
        for moving_worker_end_pos in moves_from_2_to_3.into_iter() {
            let winning_move = GenericMove::new_mortal_winning_move(
                moving_worker_start_mask,
                BitBoard::as_mask(moving_worker_end_pos),
            );
            result.push(winning_move);
            if F & STOP_ON_MATE != 0 {
                return result;
            }
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let moves_from_level_3 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & board.height_map[2] & !board.height_map[3],
        ) & !board.height_map[3];
        let moves_from_level_1 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & board.height_map[0] & !board.height_map[1],
        ) & !board.height_map[2];
        let moves_from_level_0 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & !board.height_map[1],
        ) & !board.height_map[1];

        let second_degree_remaining_moves =
            (moves_from_level_0 | moves_from_level_1 | moves_from_level_2 | moves_from_level_3)
                & !(already_counted_as_wins_mask | all_workers_mask);

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in second_degree_remaining_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let baseline_score = baseline_score
                + GRID_POSITION_SCORES[moving_worker_end_pos as usize]
                + WORKER_HEIGHT_SCORES[worker_end_height as usize];

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_mortal_move(
                    moving_worker_start_mask,
                    moving_worker_end_mask,
                    worker_build_pos,
                );
                // Instead of checks, just try to make the pieces mobile. Not sure if this is good
                // or what
                if F & INCLUDE_SCORE != 0 {
                    let second_degree_moves =
                        move_all_workers_one_include_original_workers(worker_builds)
                            & !board.height_map[3];

                    let mobility_score = (second_degree_moves
                        & board.height_map[worker_end_height]
                        & !board.height_map[std::cmp::min(3, worker_end_height + 2)])
                    .0
                    .count_ones();

                    new_action.set_score(baseline_score + (mobility_score * 3) as u8);
                }
                result.push(new_action);
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

    #[test]
    fn test_artemis_basic() {
        let state = FullGameState::try_from("0000022222000000000000000/1/artemis:0,1/mortal:23,24")
            .unwrap();

        let next_states = state.get_next_states_interactive();
        // for state in next_states {
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
        assert_eq!(next_states.len(), 1);
        assert_eq!(next_states[0].state.board.get_winner(), Some(Player::One))
    }

    #[test]
    fn test_artemis_win_check() {
        // Regular 1>2>3
        assert_eq!(
            (GodName::Artemis.to_power().get_win)(
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
            (GodName::Artemis.to_power().get_win)(
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
            (GodName::Artemis.to_power().get_win)(
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
            (GodName::Artemis.to_power().get_win)(
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
            (GodName::Artemis.to_power().get_win)(
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
            (GodName::Artemis.to_power().get_win)(
                &FullGameState::try_from("33000 44444 44444 44444 44444/1/artemis:0/mortal:24")
                    .unwrap()
                    .board,
                Player::One
            )
            .len(),
            0
        );
    }
}
