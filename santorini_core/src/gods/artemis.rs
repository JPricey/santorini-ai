use crate::{
    board::{BitmapType, BoardState, IS_WINNER_MASK, NEIGHBOR_MAP, Player, position_to_coord},
    utils::{MAIN_SECTION_MASK, move_all_workers_one_include_original_workers},
};

use super::{
    BoardStateWithAction, FullChoiceMapper, GodName, GodPower, PartialAction, StateOnlyMapper,
    mortal::mortal_player_advantage,
};

fn artemis_next_states<T, M>(state: &BoardState, player: Player) -> Vec<T>
where
    M: super::ResultsMapper<T>,
{
    let mut result: Vec<T> = Vec::with_capacity(128);
    let mut root_workers = state.workers[player as usize];
    let all_workers_mask = state.workers[0] | state.workers[1];
    let not_all_workers_mask = !all_workers_mask;

    let exactly_level_3 = state.height_map[2] & !state.height_map[3];

    while root_workers != 0 {
        let mut already_counted_as_wins_mask = 0;

        let moving_worker_start_pos = root_workers.trailing_zeros() as usize;
        let moving_worker_start_mask: BitmapType = 1 << moving_worker_start_pos;
        root_workers ^= moving_worker_start_mask;

        let mut mapper = M::new();
        mapper.add_action(PartialAction::SelectWorker(position_to_coord(
            moving_worker_start_pos,
        )));

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let worker_starting_height = state.get_height_for_worker(moving_worker_start_mask);

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_first_degree_moves = NEIGHBOR_MAP[moving_worker_start_pos]
            & !state.height_map[too_high]
            & not_all_workers_mask;

        if worker_starting_height == 2 {
            let mut level_3_neighbors = worker_first_degree_moves & exactly_level_3;
            worker_first_degree_moves ^= level_3_neighbors;
            already_counted_as_wins_mask = level_3_neighbors;

            while level_3_neighbors != 0 {
                let dest_pos = level_3_neighbors.trailing_zeros();
                level_3_neighbors ^= 1 << dest_pos;
                let mut mapper = mapper.clone();
                mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                    dest_pos as usize,
                )));

                let mut winning_next_state = state.clone();
                winning_next_state.workers[player as usize] ^=
                    moving_worker_start_mask | (1 << dest_pos) | IS_WINNER_MASK;
                winning_next_state.flip_current_player();
                result.push(mapper.map_result(winning_next_state));
            }
        }

        let moves_from_level_3 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & state.height_map[2] & !state.height_map[3],
        ) & !state.height_map[3];
        let moves_from_level_2 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & state.height_map[1] & !state.height_map[2],
        ) & !state.height_map[3];
        let moves_from_level_1 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & state.height_map[0] & !state.height_map[1],
        ) & !state.height_map[2];
        let moves_from_level_0 = move_all_workers_one_include_original_workers(
            worker_first_degree_moves & !state.height_map[1],
        ) & !state.height_map[1];

        let mut moves_from_2_to_3 = moves_from_level_2
            & exactly_level_3
            & !already_counted_as_wins_mask
            & not_all_workers_mask;
        already_counted_as_wins_mask |= moves_from_2_to_3;
        while moves_from_2_to_3 != 0 {
            let dest_pos = moves_from_2_to_3.trailing_zeros();
            moves_from_2_to_3 ^= 1 << dest_pos;
            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                dest_pos as usize,
            )));

            let mut winning_next_state = state.clone();
            winning_next_state.workers[player as usize] ^=
                moving_worker_start_mask | (1 << dest_pos) | IS_WINNER_MASK;
            winning_next_state.flip_current_player();
            result.push(mapper.map_result(winning_next_state));
        }

        let mut second_degree_remaining_moves =
            (moves_from_level_0 | moves_from_level_1 | moves_from_level_2 | moves_from_level_3)
                & &!already_counted_as_wins_mask
                & not_all_workers_mask;
        while second_degree_remaining_moves != 0 {
            let dest_pos = second_degree_remaining_moves.trailing_zeros();
            second_degree_remaining_moves ^= 1 << dest_pos;
            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                dest_pos as usize,
            )));

            let mut worker_builds =
                NEIGHBOR_MAP[dest_pos as usize] & !non_selected_workers & !state.height_map[3];

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
                next_state.workers[player as usize] ^= moving_worker_start_mask | 1 << dest_pos;
                result.push(mapper.map_result(next_state))
            }
        }
    }

    if result.len() == 0 {
        // Lose due to no moves
        let mut next_state = state.clone();
        next_state.workers[1 - player as usize] |= IS_WINNER_MASK;
        next_state.flip_current_player();
        let mut mapper = M::new();
        mapper.add_action(PartialAction::NoMoves);
        result.push(mapper.map_result(next_state));
    }

    result
}

pub fn artemis_has_win(state: &BoardState, player: Player) -> bool {
    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & MAIN_SECTION_MASK;

    let level123_workers = starting_current_workers & (state.height_map[0] & !state.height_map[3]);
    let exactly_level_2_buildings = state.height_map[1] & !state.height_map[2];

    let level_2_after_01_moves =
        move_all_workers_one_include_original_workers(level123_workers) & exactly_level_2_buildings;
    let moves_from_level_2 = move_all_workers_one_include_original_workers(level_2_after_01_moves);

    let open_spaces = !(state.workers[0] | state.workers[1] | state.height_map[3]);

    let exactly_level_3 = state.height_map[2] & open_spaces;
    let level_3_moves = moves_from_level_2 & exactly_level_3;

    level_3_moves != 0
}

pub const fn build_artemis() -> GodPower {
    GodPower {
        god_name: GodName::Artemis,
        player_advantage_fn: mortal_player_advantage,
        next_states: artemis_next_states::<BoardState, StateOnlyMapper>,
        // next_state_with_scores_fn: get_next_states_custom::<StateWithScore, HueristicMapper>,
        next_states_interactive: artemis_next_states::<BoardStateWithAction, FullChoiceMapper>,
        has_win: artemis_has_win,
    }
}

#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, gods::tests::assert_has_win_consistency};

    use super::*;

    #[test]
    #[ignore]
    fn test_artemis_basic() {
        let state_str = "0000022222000000000000000/1/artemis:0,1/mortal:23,24";
        let state = FullGameState::try_from(state_str).unwrap();

        let next_states = state.get_next_states_interactive();
        for state in next_states {
            state.state.print_to_console();
            println!("{:?}", state.actions);
        }
    }

    #[test]
    fn test_artemis_cant_move_through_wins() {
        let state_str = "2300044444000000000000000/1/artemis:0/mortal:24";
        let state = FullGameState::try_from(state_str).unwrap();
        let next_states = state.get_next_states_interactive();
        assert_eq!(next_states.len(), 1);
    }

    // #[test]
    // fn test_artemis_climb_ladder() {
    //     let state_str = "1100000200000000100000210/1/artemis:17/mortal:1,16";
    //     let state = FullGameState::try_from(state_str).unwrap();
    //     let next_states = state.get_next_states_interactive();
    //     for state in next_states {
    //         state.state.print_to_console();
    //         println!("{:?}", state.actions);
    //     }
    // }

    #[test]
    fn test_artemis_win_check() {
        // Regular 1>2>3
        assert_has_win_consistency(
            &FullGameState::try_from("12300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            true,
        );

        // Can't move 1>3
        assert_has_win_consistency(
            &FullGameState::try_from("13300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            false,
        );

        // Can move 2>2>3
        assert_has_win_consistency(
            &FullGameState::try_from("22300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            true,
        );

        // Can't move 2>1>3
        assert_has_win_consistency(
            &FullGameState::try_from("21300 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            false,
        );

        // Single move 2>3
        assert_has_win_consistency(
            &FullGameState::try_from("23000 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            true,
        );

        // Can't win from 3>3
        assert_has_win_consistency(
            &FullGameState::try_from("33000 44444 44444 44444 44444/1/artemis:0/mortal:24")
                .unwrap(),
            false,
        );
    }
}
