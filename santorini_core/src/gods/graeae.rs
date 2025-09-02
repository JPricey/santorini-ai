use crate::{
    add_scored_move,
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::FullGameState,
    build_god_power_movers, build_parse_flags, build_push_winning_moves,
    gods::{
        build_god_power_actions, generic::{MoveGenFlags, ScoredMove}, god_power, mortal::MortalMove, GodName, GodPower
    },
    player::Player,
    variable_prelude,
};

fn graeae_move_gen<const F: MoveGenFlags>(
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
       own_workers:  own_workers,
       other_workers:  other_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers:  acting_workers,
       checkable_worker_positions_mask:  checkable_worker_positions_mask,
    );

    for worker_start_pos in acting_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_starting_height = board.get_height(worker_start_pos);
        let other_own_workers = own_workers ^ moving_worker_start_mask;

        let mut other_threatening_neighbors = BitBoard::EMPTY;
        let mut other_all_neighbors = BitBoard::EMPTY;
        for other_pos in other_own_workers {
            other_all_neighbors |= NEIGHBOR_MAP[other_pos as usize];
            if board.get_height(other_pos) == 2 {
                other_threatening_neighbors |= NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if is_mate_only || worker_starting_height == 2 {
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

        let non_moved_workers = all_workers_mask ^ moving_worker_start_mask;
        let unblocked_squares = !(non_moved_workers | domes);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;

            let buildable_squares = unblocked_squares & !moving_worker_end_mask;

            let mut worker_builds = other_all_neighbors & buildable_squares;
            let worker_plausible_next_moves =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            worker_builds &= build_mask;

            if is_interact_with_key_squares {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let is_now_lvl_2 = (worker_end_height == 2) as usize;
            let reach_board = if is_against_hypnus
                && ((other_own_workers & exactly_level_2).count_ones() as usize + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                    & win_mask
            };

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = MortalMove::new_basic_move(
                    worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3 & buildable_squares;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    }

    result
}

pub const fn build_graeae() -> GodPower {
    god_power(
        GodName::Graeae,
        build_god_power_movers!(graeae_move_gen),
        build_god_power_actions::<MortalMove>(),
        3621759432554562343,
        8641066751388211347,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_num_workers(3)
}
