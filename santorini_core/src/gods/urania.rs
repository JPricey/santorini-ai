use crate::{
    add_scored_move,
    bitboard::{BitBoard, WRAPPING_NEIGHBOR_MAP, apply_mapping_to_mask},
    board::FullGameState,
    build_god_power_movers, build_parse_flags, build_push_winning_moves,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove},
        god_power,
        harpies::urania_slide,
        mortal::MortalMove,
    },
    player::Player,
    variable_prelude,
};

fn urania_move_gen<const F: MoveGenFlags>(
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

    variable_prelude!(
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
       is_against_harpies: is_against_harpies,
       own_workers:  own_workers,
       oppo_workers:  oppo_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers:  acting_workers,
       checkable_worker_positions_mask:  checkable_worker_positions_mask,
    );

    let mut null_build_blocker = BitBoard::MAIN_SECTION_MASK;

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_start_height = board.get_height(worker_start_pos);

        let other_own_workers = own_workers ^ worker_start_mask;
        let other_threatening_workers = other_own_workers & checkable_worker_positions_mask;

        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &WRAPPING_NEIGHBOR_MAP);

        let mut worker_moves = WRAPPING_NEIGHBOR_MAP[worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_start_height)]
                | all_workers_mask);

        if is_mate_only || worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & exactly_level_3 & win_mask;
            build_push_winning_moves!(
                moves_to_level_3,
                worker_moves,
                MortalMove::new_winning_move,
                worker_start_pos,
                result,
                is_stop_on_mate,
            );
        }

        if is_mate_only {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ worker_start_mask;
        let buildable_squares = !(non_selected_workers | domes);
        let mut already_seen = BitBoard::EMPTY;

        for mut worker_end_pos in worker_moves.into_iter() {
            let worker_end_mask;
            if is_against_harpies {
                worker_end_pos = urania_slide(
                    &board,
                    worker_start_pos,
                    worker_end_pos,
                    non_selected_workers,
                );
                worker_end_mask = BitBoard::as_mask(worker_end_pos);

                if (worker_end_mask & already_seen).is_not_empty() {
                    continue;
                }
                already_seen |= worker_end_mask;
            } else {
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_height;
            let not_own_workers = !(other_own_workers | worker_end_mask);
            let is_now_lvl_2 = (worker_end_height == 2) as usize;

            let mut worker_builds =
                WRAPPING_NEIGHBOR_MAP[worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;
            worker_builds &= build_mask;

            if is_interact_with_key_squares {
                if (worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            if is_stop_on_mate && worker_end_pos == worker_start_pos {
                worker_builds &= null_build_blocker;
                null_build_blocker ^= worker_builds;
            }

            let reach_board = if is_against_hypnus
                && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                    & win_mask
                    & buildable_squares
            };

            for worker_build_pos in worker_builds {
                let new_action =
                    MortalMove::new_basic_move(worker_start_pos, worker_end_pos, worker_build_pos);

                let is_check = {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = ((exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask))
                        & not_own_workers;
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
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
}
