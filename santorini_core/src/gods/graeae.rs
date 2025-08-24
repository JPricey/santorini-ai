use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            GenericMove, GodMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK,
            MATE_ONLY, MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, NULL_MOVE_DATA,
            POSITION_WIDTH, STOP_ON_MATE, ScoredMove,
        },
        mortal::MortalMove,
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

fn graeae_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
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
        let other_own_workers = current_workers ^ moving_worker_start_mask;

        let mut neighbor_neighbor = BitBoard::EMPTY;
        let mut l2_neighbor_neighbor = BitBoard::EMPTY;
        for other_pos in other_own_workers {
            neighbor_neighbor |= NEIGHBOR_MAP[other_pos as usize];
            if F & INCLUDE_SCORE != 0 && board.get_height(other_pos) == 2 {
                l2_neighbor_neighbor |= NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    MortalMove::new_mortal_winning_move(
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

            let buildable_squares = buildable_squares & !moving_worker_end_mask;

            let mut worker_builds = neighbor_neighbor & buildable_squares;
            let worker_plausible_next_moves =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let reach_board = l2_neighbor_neighbor
                | (worker_plausible_next_moves
                    & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]);

            for worker_build_pos in worker_builds {
                let new_action = MortalMove::new_mortal_move(
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

build_god_power!(
    build_graeae,
    god_name: GodName::Graeae,
    move_type: MortalMove,
    move_gen: graeae_move_gen,
    hash1: 13716661772054342839,
    hash2: 15637952489637380097,
);

#[cfg(test)]
mod tests {
    use crate::random_utils::GameStateFuzzer;

    use super::*;

    #[test]
    fn test_mortal_check_detection() {
        let mortal = GodName::Mortal.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = mortal.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = mortal.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                mortal.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    mortal.get_winning_moves(&board, current_player).len() > 0;

                if is_check_move != is_winning_next_turn {
                    println!(
                        "Failed check detection. Check guess: {:?}. Actual: {:?}",
                        is_check_move, is_winning_next_turn
                    );
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(is_check_move, is_winning_next_turn);
                }
            }
        }
    }
}
