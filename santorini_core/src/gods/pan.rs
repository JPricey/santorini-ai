use crate::{
    add_scored_move,
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::FullGameState,
    build_building_masks, build_god_power_movers, build_parse_flags, build_push_winning_moves,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
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
       state:  state,
       player:  player,
       board:  board,
       other_player:  other_player,
       current_player_idx:  current_player_idx,
       other_player_idx:  other_player_idx,
       other_god:  other_god,
       exactly_level_0:  exactly_level_0,
       exactly_level_1:  exactly_level_1,
       exactly_level_2:  exactly_level_2,
       exactly_level_3:  exactly_level_3,
       domes:  domes,
       win_mask:  win_mask,
       build_mask: build_mask,
       is_against_hypnus: is_against_hypnus,
       own_workers:  own_workers,
       oppo_workers:  oppo_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers: acting_workers,
    );

    let checkable_worker_positions_mask = board.at_least_level_2();
    if is_mate_only {
        acting_workers &= checkable_worker_positions_mask;
    }

    for moving_worker_start_pos in acting_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let other_lvl_2_workers = (own_workers ^ moving_worker_start_mask) & exactly_level_2;
        let neighbor_2 = apply_mapping_to_mask(other_lvl_2_workers, &NEIGHBOR_MAP);
        let neighbor_3 = apply_mapping_to_mask(
            (own_workers ^ moving_worker_start_mask) & exactly_level_3,
            &NEIGHBOR_MAP,
        );

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if worker_starting_height == 2 {
            let winning_moves = worker_moves & (exactly_level_0 | exactly_level_3) & win_mask;
            build_push_winning_moves!(
                winning_moves,
                worker_moves,
                MortalMove::new_winning_move,
                moving_worker_start_pos,
                result,
                is_stop_on_mate,
            );
        } else if worker_starting_height == 3 {
            let winning_moves = worker_moves & (exactly_level_0 | exactly_level_1) & win_mask;
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
            let is_now_lvl_2 = (worker_end_height == 2) as usize;

            build_building_masks!(
                worker_end_pos: moving_worker_end_pos,
                open_squares: buildable_squares,
                build_mask: build_mask,
                is_interact_with_key_squares: is_interact_with_key_squares,
                key_squares_expr: (moving_worker_end_mask & key_squares).is_empty(),
                key_squares: key_squares,

                all_possible_builds: all_possible_builds,
                narrowed_builds: narrowed_builds,
                worker_plausible_next_moves: worker_plausible_next_moves,
            );

            let mut reach_2 = neighbor_2;
            let mut reach_3 = neighbor_3;

            // dont worry about reach 3 vs hypnus because it's not possible to get up there
            if is_against_hypnus && (other_lvl_2_workers.count_ones() as usize + is_now_lvl_2) < 2 {
                reach_2 = BitBoard::EMPTY
            } else if worker_end_height == 2 {
                reach_2 |= worker_plausible_next_moves;
            } else if worker_end_height == 3 {
                reach_3 |= worker_plausible_next_moves;
            }

            for worker_build_pos in narrowed_builds {
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
                        & !moving_worker_end_mask
                        & win_mask;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    }

    result
}

pub const fn build_pan() -> GodPower {
    god_power(
        GodName::Pan,
        build_god_power_movers!(pan_move_gen),
        build_god_power_actions::<MortalMove>(),
        9244705180822939865,
        18175931309899694692,
    )
}
