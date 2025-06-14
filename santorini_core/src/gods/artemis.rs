use crate::board::{
    BitmapType, BoardState, IS_WINNER_MASK, MAIN_SECTION_MASK, NEIGHBOR_MAP, Player,
    position_to_coord,
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
        let mut all_worker_moves =
            NEIGHBOR_MAP[moving_worker_start_pos] & !state.height_map[too_high] & !all_workers_mask;

        // Compute 2nd moves by moving again from all possible 1st moves
        let mut first_level_worker_moves = all_worker_moves;
        while first_level_worker_moves != 0 {
            let first_order_pos = first_level_worker_moves.trailing_zeros();
            let first_order_mask = 1 << first_order_pos;
            first_level_worker_moves ^= first_order_mask;

            let first_order_too_high =
                std::cmp::min(3, state.get_height_for_worker(first_order_mask) + 1);

            let second_order_moves =
                NEIGHBOR_MAP[first_order_pos as usize] & !state.height_map[first_order_too_high];
            all_worker_moves |= second_order_moves;
        }
        all_worker_moves &= !all_workers_mask;

        while all_worker_moves != 0 {
            let worker_move_pos = all_worker_moves.trailing_zeros() as usize;
            let worker_move_mask: BitmapType = 1 << worker_move_pos;
            all_worker_moves ^= worker_move_mask;

            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                worker_move_pos,
            )));

            // If we just won - end now and don't build
            if state.height_map[2] & worker_move_mask > 0 {
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

pub const fn build_artemis() -> GodPower {
    GodPower {
        god_name: GodName::Artemis,
        player_advantage_fn: mortal_player_advantage,
        next_states: artemis_next_states::<BoardState, StateOnlyMapper>,
        // next_state_with_scores_fn: get_next_states_custom::<StateWithScore, HueristicMapper>,
        next_states_interactive: artemis_next_states::<BoardStateWithAction, FullChoiceMapper>,
    }
}

#[cfg(test)]
mod tests {
    use crate::board::FullGameState;

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
    fn test_artemis_2nd_order_height() {
        let state_str = "2230044444000000000000000/1/artemis:0/mortal:23,24";
        let state = FullGameState::try_from(state_str).unwrap();

        let next_states = state.get_next_states_interactive();
        for state in next_states {
            if state.state.board.get_winner().is_some() {
                return;
            }
            // state.state.print_to_console();
            // println!("{:?}", state.actions);
        }

        assert!(false, "Didn't find winning state");
    }
}
