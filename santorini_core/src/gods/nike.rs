use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        GodName, GodPower,
        athena::AthenaMove,
        build_god_power_actions,
        generic::{MoveGenFlags, ScoredMove, get_default_parse_data_err},
        god_power,
        harpies::slide_position,
        move_helpers::{
            WorkerEndMoveState, build_scored_move, get_generator_prelude_state,
            get_standard_reach_board, get_worker_next_build_state_with_is_matched,
            get_worker_next_move_state, get_worker_start_move_state, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
};

pub(super) fn nike_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(nike_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AthenaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                AthenaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for mut worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_height;
            let is_improving;
            let is_power_enabled;
            if prelude.is_against_harpies {
                is_power_enabled = prelude.board.get_height(worker_end_pos)
                    < worker_start_state.worker_start_height;
                worker_end_pos = slide_position(
                    &prelude,
                    worker_start_state.worker_start_pos,
                    worker_end_pos,
                );

                worker_end_height = prelude.board.get_height(worker_end_pos);
                is_improving = worker_end_height > worker_start_state.worker_start_height;
            } else {
                worker_end_height = prelude.board.get_height(worker_end_pos);
                is_improving = worker_end_height > worker_start_state.worker_start_height;
                is_power_enabled = worker_end_height < worker_start_state.worker_start_height;
            }

            let worker_end_move_state = WorkerEndMoveState {
                worker_end_pos,
                worker_end_height,
                is_improving,
                worker_end_mask: BitBoard::as_mask(worker_end_pos),
                is_now_lvl_2: (worker_end_height == 2) as u32,
            };

            let worker_next_build_state = get_worker_next_build_state_with_is_matched::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
                is_power_enabled
                    || (worker_end_move_state.worker_end_mask & key_squares).is_not_empty(),
            );

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = AthenaMove::new_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    is_power_enabled,
                );

                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    result
}

fn nike_passing_move(board: &mut BoardState) {
    board.set_god_data(board.current_player, 0);
}

fn can_opponent_climb(board: &BoardState, player: Player) -> bool {
    board.god_data[player as usize] == 0
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "^" => Ok(1),
        "" => Ok(0),
        _ => get_default_parse_data_err(data),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        _ => Some("^".to_owned()),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    if board.current_player == player {
        return None;
    }
    let god_data = board.god_data[player as usize];
    match god_data {
        0 => None,
        _ => Some("Preventing Upward Moves".to_owned()),
    }
}

pub const fn build_nike() -> GodPower {
    god_power(
        GodName::Nike,
        build_god_power_movers!(nike_move_gen),
        build_god_power_actions::<AthenaMove>(),
        2166638488424994940,
        8591575656066204147,
    )
    .with_make_passing_move_fn(nike_passing_move)
    .with_can_opponent_climb_fn(can_opponent_climb)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}
