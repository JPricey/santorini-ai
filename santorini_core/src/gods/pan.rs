use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        GodName, GodPower,
        generic::{
            INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, MoveGenFlags, STOP_ON_MATE,
            ScoredMove,
        },
        mortal::MortalMove,
    },
    player::Player,
};

fn pan_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let board = &state.board;
    let current_player_idx = player as usize;
    let exactly_level_0 = board.exactly_level_0();
    let exactly_level_1 = board.exactly_level_1();
    let exactly_level_2 = board.exactly_level_2();
    let exactly_level_3 = board.exactly_level_3();

    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.at_least_level_2();
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_2 = BitBoard::EMPTY;
        let mut neighbor_3 = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            for other_pos_2 in (current_workers ^ moving_worker_start_mask) & exactly_level_2 {
                neighbor_2 |= NEIGHBOR_MAP[other_pos_2 as usize];
            }

            for other_pos_3 in (current_workers ^ moving_worker_start_mask) & exactly_level_3 {
                neighbor_3 |= NEIGHBOR_MAP[other_pos_3 as usize];
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if worker_starting_height == 2 {
            let winning_moves = worker_moves & (exactly_level_0 | exactly_level_3);
            worker_moves ^= winning_moves;

            for moving_worker_end_pos in winning_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    MortalMove::new_winning_move(
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
            let winning_moves = worker_moves & exactly_level_0 | exactly_level_1;
            worker_moves ^= winning_moves;

            for moving_worker_end_pos in winning_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    MortalMove::new_winning_move(
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
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut reach_2 = neighbor_2;
            let mut reach_3 = neighbor_3;
            if worker_end_height == 2 {
                reach_2 |= worker_plausible_next_moves;
            } else if worker_end_height == 3 {
                reach_3 |= worker_plausible_next_moves;
            }

            for worker_build_pos in worker_builds {
                let new_action = MortalMove::new_basic_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_0 = exactly_level_0 & !worker_build_mask;
                    let final_level_1 = exactly_level_0 | exactly_level_1 & !worker_build_mask;
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);

                    let check_board = ((reach_2 & (final_level_0 | final_level_3))
                        | (reach_3 & (final_level_0 | final_level_1)))
                        & buildable_squares
                        & !moving_worker_end_mask;
                    let is_check = check_board.is_not_empty();

                    if is_check {
                        result.push(ScoredMove::new_checking_move(new_action.into()));
                    } else {
                        let is_improving = worker_end_height > worker_starting_height;
                        if is_improving {
                            result.push(ScoredMove::new_improving_move(new_action.into()));
                        } else {
                            result.push(ScoredMove::new_non_improver(new_action.into()));
                        };
                    }
                } else {
                    result.push(ScoredMove::new_unscored_move(new_action.into()));
                }
            }
        }
    }

    result
}

build_god_power!(
    build_pan,
    god_name: GodName::Pan,
    move_type: MortalMove,
    move_gen: pan_move_gen,
    hash1: 9244705180822939865,
    hash2: 18175931309899694692,
);
