use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        FullAction, GodName, GodPower, PartialAction,
        generic::{
            GRID_POSITION_SCORES, GenericMove, INCLUDE_SCORE, MATE_ONLY,
            MoveData, MoveGenFlags, POSITION_WIDTH, RETURN_FIRST_MATE, STOP_ON_MATE,
            WORKER_HEIGHT_SCORES,
        },
        mortal::MORTAL_BUILD_POSITION_OFFSET,
    },
    player::Player,
    square::Square,
};

const HEPH_DOUBLE_BUILD_MASK: MoveData = 1 << (MORTAL_BUILD_POSITION_OFFSET + POSITION_WIDTH);

pub fn hephaestus_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let current_player = board.current_player;
    let worker_move_mask = action.mortal_move_mask();
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers.0 & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    let mut action_vec = vec![
        PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
        PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
    ];

    if action.get_is_winning() {
        return vec![action_vec];
    }

    let build_position = action.mortal_build_position();
    action_vec.push(PartialAction::Build(Square::from(build_position as usize)));

    if action.data & HEPH_DOUBLE_BUILD_MASK > 0 {
        action_vec.push(PartialAction::Build(Square::from(build_position as usize)));
    }

    return vec![action_vec];
}

pub fn hephaestus_make_move(board: &mut BoardState, action: GenericMove) {
    let worker_move_mask = action.mortal_move_mask();
    board.workers[board.current_player as usize].0 ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    let build_position = action.mortal_build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    for height in 0..4 {
        if (board.height_map[height] & build_mask).is_empty() {
            board.height_map[height] ^= build_mask;

            if action.data & HEPH_DOUBLE_BUILD_MASK > 0 {
                board.height_map[height + 1] ^= build_mask;
            }
            return;
        }
    }
    panic!("Expected to build, but couldn't")
}

pub fn hephaestus_unmake_move(board: &mut BoardState, action: GenericMove) {
    let worker_move_mask = action.mortal_move_mask();
    board.workers[board.current_player as usize].0 ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner(board.current_player);
        return;
    }

    let build_position = action.mortal_build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    for height in (0..4).rev() {
        if (board.height_map[height] & build_mask).is_not_empty() {
            board.height_map[height] ^= build_mask;

            if action.data & HEPH_DOUBLE_BUILD_MASK > 0 {
                board.height_map[height - 1] ^= build_mask;
            }
            break;
        }
    }
}

fn hephaestus_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
) -> Vec<GenericMove> {
    let mut result = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let current_workers = starting_current_workers;

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let baseline_score = 50
            - GRID_POSITION_SCORES[moving_worker_start_pos as usize]
            - WORKER_HEIGHT_SCORES[worker_starting_height];

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        if worker_starting_height != 3 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

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
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let baseline_score = baseline_score
                + GRID_POSITION_SCORES[moving_worker_end_pos as usize]
                + WORKER_HEIGHT_SCORES[worker_end_height as usize];

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            let (
                check_count,
                builds_that_result_in_checks,
                double_builds_that_result_in_checks,
                build_that_remove_checks,
            ) = if worker_end_height == 2 {
                let exactly_level_1 = board.height_map[0] & !board.height_map[1];
                let level_3 = board.height_map[2];
                let check_count = (worker_builds & level_3).0.count_ones();
                let exactly_level_2 = board.height_map[1] & !board.height_map[2];
                let builds_that_result_in_checks = worker_builds & exactly_level_2;
                let builds_that_remove_checks = worker_builds & level_3;
                (
                    check_count as u8,
                    builds_that_result_in_checks,
                    worker_builds & exactly_level_1,
                    builds_that_remove_checks,
                )
            } else {
                (0, BitBoard::EMPTY, BitBoard::EMPTY, BitBoard::EMPTY)
            };

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_mortal_move(
                    moving_worker_start_mask,
                    moving_worker_end_mask,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let check_count = check_count
                        + ((builds_that_result_in_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as u8)
                        - ((build_that_remove_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as u8);
                    new_action.set_score(baseline_score + check_count * 30);
                }
                result.push(new_action);

                if (BitBoard::as_mask(worker_build_pos) & !board.height_map[1]).is_not_empty() {
                    new_action.data |= HEPH_DOUBLE_BUILD_MASK;

                    if F & INCLUDE_SCORE != 0 {
                        let check_count = check_count
                            + ((double_builds_that_result_in_checks
                                & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as u8)
                            - ((build_that_remove_checks & BitBoard::as_mask(worker_build_pos))
                                .is_not_empty() as u8);
                        new_action.set_score(baseline_score + check_count * 31);
                    }

                    result.push(new_action);
                }
            }
        }
    }

    result
}

pub const fn build_hephaestus() -> GodPower {
    GodPower {
        god_name: GodName::Hephaestus,
        get_all_moves: hephaestus_move_gen::<0>,
        get_moves: hephaestus_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        get_win: hephaestus_move_gen::<{ RETURN_FIRST_MATE }>,
        get_actions_for_move: hephaestus_move_to_actions,
        _make_move: hephaestus_make_move,
        _unmake_move: hephaestus_unmake_move,
    }
}

/*
pub fn hephaestus_next_states<T, M>(state: &BoardState, player: Player) -> Vec<T>
where
    M: super::ResultsMapper<T>,
{
    let mut result: Vec<T> = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & MAIN_SECTION_MASK;
    let mut current_workers = starting_current_workers;

    let all_workers_mask = state.workers[0] | state.workers[1];

    while current_workers != 0 {
        let moving_worker_start_pos = current_workers.trailing_zeros() as usize;
        let moving_worker_start_mask: BitmapType = 1 << moving_worker_start_pos;
        current_workers ^= moving_worker_start_mask;

        let mut mapper = M::new();
        mapper.add_action(PartialAction::SelectWorker(position_to_coord(
            moving_worker_start_pos,
        )));

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let worker_starting_height = state.get_height_for_worker(moving_worker_start_mask);

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos]
            & !state.height_map[too_high]
            & !non_selected_workers;

        while worker_moves != 0 {
            let worker_move_pos = worker_moves.trailing_zeros() as usize;
            let worker_move_mask: BitmapType = 1 << worker_move_pos;
            worker_moves ^= worker_move_mask;

            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                worker_move_pos,
            )));

            if state.height_map[2] & worker_move_mask > 0 {
                let mut winning_next_state = state.clone();
                winning_next_state.workers[current_player_idx] ^=
                    moving_worker_start_mask | worker_move_mask | IS_WINNER_MASK;
                winning_next_state.flip_current_player();
                result.push(mapper.map_result(winning_next_state));
                continue;
            }

            let mut worker_builds =
                NEIGHBOR_MAP[worker_move_pos] & !non_selected_workers & !state.height_map[3];

            while worker_builds != 0 {
                let worker_build_pos = worker_builds.trailing_zeros() as usize;
                let worker_build_mask = 1 << worker_build_pos;
                worker_builds ^= worker_build_mask;

                let mut mapper = mapper.clone();
                mapper.add_action(PartialAction::Build(position_to_coord(worker_build_pos)));

                let mut next_state = state.clone();
                next_state.flip_current_player();
                for height in 0.. {
                    if next_state.height_map[height] & worker_build_mask == 0 {
                        next_state.height_map[height] |= worker_build_mask;

                        next_state.workers[current_player_idx] ^=
                            moving_worker_start_mask | worker_move_mask;
                        result.push(mapper.map_result(next_state.clone()));

                        // Maybe build again
                        if height < 2 {
                            next_state.height_map[height + 1] |= worker_build_mask;
                            mapper.add_action(PartialAction::Build(position_to_coord(
                                worker_build_pos,
                            )));
                            result.push(mapper.map_result(next_state.clone()))
                        }
                        break;
                    }
                }
            }
        }
    }

    if result.len() == 0 {
        // Lose due to no moves
        let mut next_state = state.clone();
        next_state.workers[1 - current_player_idx] |= IS_WINNER_MASK;
        next_state.flip_current_player();
        let mut mapper = M::new();
        mapper.add_action(PartialAction::NoMoves);
        result.push(mapper.map_result(next_state));
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::board::FullGameState;

    #[test]
    fn test_hephaestus_basic() {
        let state_str = "0010044444000000000000000/1/hephaestus:0/mortal:23,24";
        let state = FullGameState::try_from(state_str).unwrap();

        let next_states = state.get_next_states_interactive();
        assert_eq!(next_states.len(), 4);

        // for state in next_states {
        //     state.state.print_to_console();
        //     println!("{:?}", state.actions);
        // }
    }

    #[test]
    fn test_hephaestus_no_extra_domes() {
        let state_str = "2240044444000000000000000/1/hephaestus:0/mortal:23,24";
        let state = FullGameState::try_from(state_str).unwrap();

        let next_states = state.get_next_states_interactive();
        assert_eq!(next_states.len(), 1);

        // for state in next_states {
        //     state.state.print_to_console();
        //     println!("{:?}", state.actions);
        // }
    }
}
*/
