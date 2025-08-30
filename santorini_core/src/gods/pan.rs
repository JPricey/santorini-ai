use crate::{
    add_scored_move,
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_god_power, build_parse_flags, build_push_winning_moves,
    gods::{
        GodName, GodPower,
        generic::{
            INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, MoveGenFlags, STOP_ON_MATE,
            ScoredMove,
        },
        mortal::MortalMove,
    },
    non_checking_variable_prelude,
    player::Player,
};

fn pan_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    build_parse_flags!(
        is_mate_only,
        is_include_score,
        is_stop_on_mate,
        is_interact_with_key_squares
    );

    non_checking_variable_prelude!(
        state,
        player,
        board,
        other_player,
        current_player_idx,
        other_player_idx,
        exactly_level_0,
        exactly_level_1,
        exactly_level_2,
        exactly_level_3,
        domes,
        own_workers,
        other_workers,
        result,
        all_workers_mask,
        is_mate_only,
    );

    let mut current_workers = own_workers;
    let checkable_worker_positions_mask = board.at_least_level_2();
    if is_mate_only {
        current_workers &= checkable_worker_positions_mask;
    }

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_2 = BitBoard::EMPTY;
        let mut neighbor_3 = BitBoard::EMPTY;
        for other_pos_2 in (current_workers ^ moving_worker_start_mask) & exactly_level_2 {
            neighbor_2 |= NEIGHBOR_MAP[other_pos_2 as usize];
        }

        for other_pos_3 in (current_workers ^ moving_worker_start_mask) & exactly_level_3 {
            neighbor_3 |= NEIGHBOR_MAP[other_pos_3 as usize];
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if worker_starting_height == 2 {
            let winning_moves = worker_moves & (exactly_level_0 | exactly_level_3);
            build_push_winning_moves!(
                winning_moves,
                worker_moves,
                MortalMove::new_winning_move,
                moving_worker_start_pos,
                result,
                is_stop_on_mate,
            );
        } else if worker_starting_height == 3 {
            let winning_moves = worker_moves & (exactly_level_0 | exactly_level_1);
            build_push_winning_moves!(
                winning_moves,
                worker_moves,
                MortalMove::new_winning_move,
                moving_worker_start_pos,
                result,
                is_stop_on_mate,
            );
        }

        if is_mate_only {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | domes);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;

            if is_interact_with_key_squares {
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
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let anti_worker_build_mask = !worker_build_mask;
                let new_action = MortalMove::new_basic_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                let winnable_from_2 = reach_2
                    & ((exactly_level_0 | exactly_level_3) & anti_worker_build_mask
                        | exactly_level_2 & worker_build_mask);
                let winnable_from_3 =
                    reach_3 & (exactly_level_0 | exactly_level_1 & anti_worker_build_mask);

                let is_check = {
                    let check_board = (winnable_from_2 | winnable_from_3)
                        & buildable_squares
                        & !moving_worker_end_mask;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
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
