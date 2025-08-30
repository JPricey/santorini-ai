use crate::{
    bitboard::BitBoard,
    board::{FullGameState, WRAPPING_NEIGHBOR_MAP},
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{
            INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, MoveGenFlags, STOP_ON_MATE,
            ScoredMove,
        },
        god_power,
        mortal::MortalMove,
    },
    player::Player,
};

fn urania_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let board = &state.board;
    let current_player_idx = player as usize;
    let exactly_level_2 = board.exactly_level_2();
    let exactly_level_3 = board.exactly_level_3();
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };
    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);
    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_neighbor = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_checkable_workers =
                (current_workers ^ moving_worker_start_mask) & exactly_level_2;
            for other_pos in other_checkable_workers {
                neighbor_neighbor |= WRAPPING_NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let mut worker_moves = WRAPPING_NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
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
                WRAPPING_NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let reach_board = neighbor_neighbor
                | (worker_plausible_next_moves
                    & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]);
            for worker_build_pos in worker_builds {
                let new_action = MortalMove::new_basic_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3 & buildable_squares;
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

pub const fn build_urania() -> GodPower {
    god_power(
        GodName::Urania,
        build_god_power_movers!(urania_move_gen),
        build_god_power_actions::<MortalMove>(),
        9064977946056493903,
        14574722042933820831,
    )
    .with_nnue_god_name(GodName::Mortal)
}
