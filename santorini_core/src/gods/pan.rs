use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        GodName, GodPower,
        generic::{
            GRID_POSITION_SCORES, GenericMove, INCLUDE_SCORE, MATE_ONLY, MoveData, MoveGenFlags,
            RETURN_FIRST_MATE, STOP_ON_MATE,
        },
        mortal::{mortal_make_move, mortal_move_to_actions, mortal_unmake_move},
    },
    player::Player,
    square::Square,
};

const PAN_BUILD_POSITION_OFFSET: usize = 25;
const PAN_HEIGHT_SCORES: [u8; 4] = [0, 10, 25, 25];

impl GenericMove {
    fn new_pan_move(
        move_from_mask: BitBoard,
        move_to_mask: BitBoard,
        build_position: Square,
    ) -> GenericMove {
        let mut data: MoveData = (move_from_mask.0 | move_to_mask.0) as MoveData;
        data |= (build_position as MoveData) << PAN_BUILD_POSITION_OFFSET;

        Self::new(data)
    }

    fn new_pan_winning_move(move_from_mask: BitBoard, move_to_mask: BitBoard) -> GenericMove {
        let data: MoveData = (move_from_mask.0 | move_to_mask.0) as MoveData;
        Self::new_winning_move(data)
    }
}

fn pan_move_gen<const F: MoveGenFlags>(board: &BoardState, player: Player) -> Vec<GenericMove> {
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
            - PAN_HEIGHT_SCORES[worker_starting_height];

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        let mut winning_moves = BitBoard::EMPTY;

        if worker_starting_height >= 2 {
            let move_to_fall_height = worker_moves & !board.height_map[worker_starting_height - 2];
            winning_moves |= move_to_fall_height;

            if worker_starting_height != 3 {
                let moves_to_level_3 = worker_moves & board.height_map[2];
                winning_moves |= moves_to_level_3;
            }
        }

        for moving_worker_end_pos in winning_moves.into_iter() {
            let winning_move = GenericMove::new_pan_winning_move(
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

        worker_moves ^= winning_moves;

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let baseline_score = baseline_score
                + GRID_POSITION_SCORES[moving_worker_end_pos as usize]
                + PAN_HEIGHT_SCORES[worker_end_height as usize];

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            let (check_count, builds_that_result_in_checks, build_that_remove_checks) =
                if worker_end_height == 2 {
                    let exactly_level_2 = board.height_map[1] & !board.height_map[2];
                    let level_3 = board.height_map[2];
                    let level_0 = !board.height_map[0];
                    // (worker_builds & exactly_level_2)
                    let check_count = (worker_builds & level_3).0.count_ones();
                    let builds_that_result_in_checks = worker_builds & exactly_level_2;
                    let builds_that_remove_checks = worker_builds & level_3 & level_0;
                    (
                        check_count as u8,
                        builds_that_result_in_checks,
                        builds_that_remove_checks,
                    )
                } else {
                    (0, BitBoard::EMPTY, BitBoard::EMPTY)
                };

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_pan_move(
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
            }
        }
    }

    result
}

pub const fn build_pan() -> GodPower {
    GodPower {
        god_name: GodName::Pan,
        get_all_moves: pan_move_gen::<0>,
        get_moves: pan_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        get_win: pan_move_gen::<{ RETURN_FIRST_MATE }>,
        get_actions_for_move: mortal_move_to_actions,
        _make_move: mortal_make_move,
        _unmake_move: mortal_unmake_move,
    }
}

#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, fen::game_state_to_fen};

    use super::*;

    #[test]
    fn test_pan_basic() {
        let state =
            FullGameState::try_from("2000044444000000000000000/1/pan:0/mortal:23,24").unwrap();
        state.print_to_console();

        let next_states = state.get_next_states_interactive();

        assert_eq!(next_states.len(), 1);
        assert_eq!(
            game_state_to_fen(&next_states[0].state),
            "2000044444000000000000000/2/#pan:1/mortal:23,24",
        );
    }

    #[test]
    fn test_pan_win_checking() {
        {
            // Regular win con
            let state =
                FullGameState::try_from("22222 22222 22232 22222 22222/1/pan:12/pan:24").unwrap();
            assert_eq!(
                (GodName::Pan.to_power().get_win)(&state.board, Player::One).len(),
                1
            );

            assert_eq!(
                (GodName::Pan.to_power().get_win)(&state.board, Player::Two).len(),
                0
            );
        }

        {
            // Fall from level 2 to 0
            let state =
                FullGameState::try_from("00000 00000 00200 00000 00030/1/pan:12/pan:13").unwrap();
            assert_eq!(
                (GodName::Pan.to_power().get_win)(&state.board, Player::One).len(),
                1
            );
        }

        {
            // Fall from 3 to 1
            let state =
                FullGameState::try_from("1111111111113111111111111/1/pan:12/pan:24").unwrap();
            assert_eq!(
                (GodName::Pan.to_power().get_win)(&state.board, Player::One).len(),
                1,
            );
        }
    }
}
