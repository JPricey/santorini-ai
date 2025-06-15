use crate::{
    board::{position_to_coord, BitmapType, BoardState, Player, IS_WINNER_MASK, NEIGHBOR_MAP},
    utils::{move_all_workers_one_exclude_original_workers, move_all_workers_one_include_original_workers, MAIN_SECTION_MASK},
};

use super::{
    BoardStateWithAction, FullChoiceMapper, GodName, GodPower, PartialAction, StateOnlyMapper,
    mortal::mortal_player_advantage,
};

pub fn pan_next_states<T, M>(state: &BoardState, player: Player) -> Vec<T>
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

        // Remember that actual height map is offset by 1
        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos]
            & !state.height_map[too_high]
            & !non_selected_workers;

        while worker_moves != 0 {
            let worker_move_pos = worker_moves.trailing_zeros() as usize;
            let worker_move_mask: BitmapType = 1 << worker_move_pos;
            worker_moves ^= worker_move_mask;

            let new_worker_height = state.get_height_for_worker(worker_move_mask);
            let pan_win = new_worker_height + 2 <= worker_starting_height;
            let regular_win = new_worker_height == 3 && worker_starting_height < 3;

            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                worker_move_pos,
            )));

            if pan_win || regular_win {
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
                        break;
                    }
                }
                next_state.workers[current_player_idx] ^=
                    moving_worker_start_mask | worker_move_mask;
                result.push(mapper.map_result(next_state))
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

pub fn pan_has_win(state: &BoardState, player: Player) -> bool {
    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & MAIN_SECTION_MASK;

    let level_2_workers = starting_current_workers & (state.height_map[1] & !state.height_map[2]);
    if level_2_workers != 0 {
        let moves_from_lvl_2 = move_all_workers_one_include_original_workers(level_2_workers);
        let open_spaces = !(state.workers[0] | state.workers[1] | state.height_map[3]);
        let level_3_or_0 = (state.height_map[2] | !state.height_map[0]) & open_spaces;
        let wins_from_level_2 = moves_from_lvl_2 & level_3_or_0;
        if wins_from_level_2 != 0 {
            return true;
        }
    }

    let level_3_workers = starting_current_workers & state.height_map[2];
    if level_3_workers != 0 {
        let moves_from_lvl_3 = move_all_workers_one_include_original_workers(level_3_workers);
        let open_spaces = !(state.workers[0] | state.workers[1] | state.height_map[3]);
        let level_0_or_1 = !state.height_map[1] & open_spaces;
        let wins_from_level_3 = moves_from_lvl_3 & level_0_or_1;
        if wins_from_level_3 != 0 {
            return true;
        }
    }

    return false;
}

pub const fn build_pan() -> GodPower {
    GodPower {
        god_name: GodName::Pan,
        player_advantage_fn: mortal_player_advantage,
        next_states: pan_next_states::<BoardState, StateOnlyMapper>,
        // next_state_with_scores_fn: get_next_states_custom::<StateWithScore, HueristicMapper>,
        next_states_interactive: pan_next_states::<BoardStateWithAction, FullChoiceMapper>,
        has_win: pan_has_win,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        board::{FullGameState, Player},
        fen::game_state_to_fen,
        gods::pan::pan_has_win,
    };

    #[test]
    fn test_pan_basic() {
        let state_str = "2000044444000000000000000/1/pan:0/mortal:23,24";
        let state = FullGameState::try_from(state_str).unwrap();

        let next_states = state.get_next_states_interactive();
        // for state in &next_states {
        //     state.state.print_to_console();
        //     println!("{:?}", state.actions);
        // }

        assert_eq!(next_states.len(), 1);
        assert_eq!(
            game_state_to_fen(&next_states[0].state),
            "2000044444000000000000000/2/#pan:1/mortal:23,24",
        );
    }

    #[test]
    fn test_pan_win_checking() {
        fn slow_win_check(state: &FullGameState) -> bool {
            let child_state = state.get_next_states();
            for child in child_state {
                if child.board.get_winner() == Some(state.board.current_player) {
                    return true;
                }
            }
            return false;
        }

        fn assert_win_checking(state: &FullGameState, expected: bool) {
            assert_eq!(
                slow_win_check(state),
                expected,
                "State fully expanded win in 1 was not as expected [{:?}] for {:?}",
                expected,
                state
            );
            assert_eq!(
                pan_has_win(&state.board, state.board.current_player),
                expected,
                "State quick check win in 1 was not as expected [{:?}] for {:?}",
                expected,
                state
            );
        }

        {
            // Regular win con
            let state_str = "22222 22222 22232 22222 22222/1/pan:12/pan:24";
            let mut state = FullGameState::try_from(state_str).unwrap();

            assert_win_checking(&state, true);
            state.board.current_player = Player::Two;
            assert_win_checking(&state, false);
        }

        {
            // Fall from level 2 to 0
            let state_str = "00000 00000 00200 00000 00030/1/pan:12/pan:13";
            let state = FullGameState::try_from(state_str).unwrap();
            assert_win_checking(&state, true);
        }

        {
            // Fall from 3 to 1
            let state_str = "1111111111113111111111111/1/pan:12/pan:24";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_win_checking(&state, true);
        }
    }
}
